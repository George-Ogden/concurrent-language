#pragma once

#include "fn/fn_gen.tpp"
#include "fn/fn_inst.tpp"
#include "lazy/fns.hpp"
#include "lazy/lazy.tpp"
#include "types/builtin.hpp"

#include <gtest/gtest.h>

#include <memory>

class PlusFn : public TypedClosureI<Empty, Int, Int, Int> {
    using TypedClosureI<Empty, Int, Int, Int>::TypedClosureI;
    LazyT<Int> body(LazyT<Int> &a, LazyT<Int> &b) override {
        return make_lazy<Int>(a->value() + b->value());
    }

  public:
    constexpr std::size_t lower_size_bound() const override { return 100; };
    constexpr std::size_t upper_size_bound() const override { return 100; };
    constexpr bool is_recursive() const override { return false; };
    static std::unique_ptr<TypedFnI<Int, Int, Int>> init(const ArgsT &args) {
        return std::make_unique<PlusFn>(args);
    }
};

TEST(FnTest, TestFnCall) {
    TypedClosureG<Empty, Int, Int, Int> plus_fn{PlusFn::init};
    std::unique_ptr<TypedFnI<Int, Int, Int>> plus_inst =
        plus_fn.init(make_lazy<Int>(3), make_lazy<Int>(4));
    ASSERT_FALSE(plus_inst->execute_immediately());
    ASSERT_EQ(plus_inst->run()->value(), 7);
}

class ClosureTest : public ::testing::Test {
  protected:
    void SetUp() override {
        ThreadManager::override_concurrency(1);
        ThreadManager::register_self(0);
        WorkRunner::setup(1);
        WorkManager::runners.clear();
        WorkManager::runners.emplace_back(std::make_unique<WorkRunner>(0));
    }

    void TearDown() override { ThreadManager::reset_concurrency_override(); }
};

class AdderFn : public TypedClosureI<Int, Int, Int> {
    using TypedClosureI<Int, Int, Int>::TypedClosureI;
    LazyT<Int> body(LazyT<Int> &a) override {
        return make_lazy<Int>(a->value() + env->value());
    }
    constexpr bool is_recursive() const override { return false; };

  public:
    constexpr std::size_t lower_size_bound() const override { return 60; };
    constexpr std::size_t upper_size_bound() const override { return 60; };
    static std::unique_ptr<TypedFnI<Int, Int>> init(const ArgsT &args,
                                                    const EnvT &env) {
        return std::make_unique<AdderFn>(args, env);
    }
};

TEST_F(ClosureTest, TestClosureCall) {
    LazyT<Int> env = make_lazy<Int>(7);
    TypedClosureG<Int, Int, Int> adder_fn{AdderFn::init, env};
    std::unique_ptr<TypedFnI<Int, Int>> adder_inst =
        adder_fn.init(make_lazy<Int>(4));
    ASSERT_GT(adder_inst->upper_size_bound(), IMMEDIATE_EXECUTION_THRESHOLD);
    ASSERT_FALSE(adder_inst->execute_immediately());
    ASSERT_EQ(adder_inst->run()->value(), 11);
}

LazyT<Int> call_closure(TypedFnG<Int, Int> &f, LazyT<Int> a) {
    return f.init(a)->run();
}

TEST_F(ClosureTest, TestFnCast) {
    LazyT<Int> env = make_lazy<Int>(4);
    TypedClosureG<Int, Int, Int> adder_fn{AdderFn::init, env};
    ASSERT_EQ(call_closure(adder_fn, make_lazy<Int>(7))->value(), 11);
}

class FibFn : public TypedClosureI<WeakFnT<Int, Int>, Int, Int> {
    using TypedClosureI<WeakFnT<Int, Int>, Int, Int>::TypedClosureI;
    LazyT<Int> body(LazyT<Int> &x) override {
        Int y = x->value();
        if (y < 0) {
            return make_lazy<Int>(0);
        } else if (y <= 1) {
            return make_lazy<Int>(1);
        } else {
            LazyT<FnT<Int, Int>> f = load_env(env);
            return make_lazy<Int>(
                f->value()->init(make_lazy<Int>(y - 1))->run()->value() +
                f->value()->init(make_lazy<Int>(y - 2))->run()->value());
        }
    }

  public:
    constexpr bool is_recursive() const override { return true; };
    constexpr std::size_t lower_size_bound() const override { return 10; };
    constexpr std::size_t upper_size_bound() const override { return 250; };
    static std::unique_ptr<TypedFnI<Int, Int>> init(const ArgsT &args,
                                                    const EnvT &env) {
        return std::make_unique<FibFn>(args, env);
    }
};

TEST_F(ClosureTest, TestRecursiveClosure) {
    LazyT<FnT<Int, Int>> fib_fn = make_lazy<FnT<Int, Int>>(
        std::make_shared<TypedClosureG<WeakFnT<Int, Int>, Int, Int>>(
            FibFn::init));
    std::dynamic_pointer_cast<TypedClosureG<WeakFnT<Int, Int>, Int, Int>>(
        fib_fn->lvalue())
        ->env = store_env<typename FibFn::EnvT>(fib_fn);

    ASSERT_EQ(call_closure(*fib_fn->lvalue(), make_lazy<Int>(5))->value(), 8);
    ASSERT_FALSE(
        fib_fn->value()->init(make_lazy<Int>(5))->execute_immediately());
}

TEST_F(ClosureTest, TestFibFnCaching) {
    LazyT<FnT<Int, Int>> fib = make_lazy<FnT<Int, Int>>(
        std::make_shared<TypedClosureG<WeakFnT<Int, Int>, Int, Int>>(
            FibFn::init));
    std::dynamic_pointer_cast<TypedClosureG<WeakFnT<Int, Int>, Int, Int>>(
        fib->lvalue())
        ->env = store_env<typename FibFn::EnvT>(fib);

    auto fib_fn = fib->value()->init(0);
    auto f_5a = fib_fn->fn_call(fib->value(), Int{5});
    auto f_4 = fib_fn->fn_call(fib->value(), Int{4});
    auto f_5b = fib_fn->fn_call(fib->value(), Int{5});

    ASSERT_EQ(f_5a, f_5b);
    ASSERT_NE(f_5a, f_4);
}

TEST_F(ClosureTest, TestPlusFnCaching) {
    LazyT<FnT<Int, Int>> fib = make_lazy<FnT<Int, Int>>(
        std::make_shared<TypedClosureG<WeakFnT<Int, Int>, Int, Int>>(
            FibFn::init));
    std::dynamic_pointer_cast<TypedClosureG<WeakFnT<Int, Int>, Int, Int>>(
        fib->lvalue())
        ->env = store_env<typename FibFn::EnvT>(fib);

    auto fib_fn = fib->value()->init(0);
    FnT<Int, Int, Int> plus_fn =
        std::make_shared<TypedClosureG<Empty, Int, Int, Int>>(PlusFn::init);
    auto plus_4_8a = fib_fn->fn_call(plus_fn, Int{4}, Int{8});
    auto plus_5_8 = fib_fn->fn_call(plus_fn, Int{5}, Int{8});
    auto plus_4_9 = fib_fn->fn_call(plus_fn, Int{4}, Int{9});
    auto plus_4_8b = fib_fn->fn_call(plus_fn, Int{4}, Int{8});

    ASSERT_EQ(plus_4_8a, plus_4_8b);
    ASSERT_NE(plus_5_8, plus_4_8a);
    ASSERT_NE(plus_4_9, plus_4_8a);
    ASSERT_NE(plus_5_8, plus_4_9);
}

TEST_F(ClosureTest, TestMixedFnCaching) {
    LazyT<FnT<Int, Int>> fib = make_lazy<FnT<Int, Int>>(
        std::make_shared<TypedClosureG<WeakFnT<Int, Int>, Int, Int>>(
            FibFn::init));
    std::dynamic_pointer_cast<TypedClosureG<WeakFnT<Int, Int>, Int, Int>>(
        fib->lvalue())
        ->env = store_env<typename FibFn::EnvT>(fib);

    auto fib_fn = fib->value()->init(0);
    FnT<Int, Int> inc_fn = std::make_shared<TypedClosureG<Int, Int, Int>>(
        AdderFn::init, make_lazy<Int>(1));
    auto fib_5 = fib_fn->fn_call(fib->value(), Int{5});
    auto inc_5 = fib_fn->fn_call(inc_fn, Int{5});

    ASSERT_NE(fib_5, inc_5);
}

struct PairIntInt : public TypedClosureI<Empty, TupleT<Int, Int>, Int, Int> {
    using TypedClosureI<Empty, TupleT<Int, Int>, Int, Int>::TypedClosureI;
    LazyT<TupleT<Int, Int>> body(LazyT<Int> &x, LazyT<Int> &y) override {
        return std::make_tuple(x, y);
    }
    constexpr std::size_t lower_size_bound() const override { return 60; };
    constexpr std::size_t upper_size_bound() const override { return 60; };
    static std::unique_ptr<TypedFnI<TupleT<Int, Int>, Int, Int>>
    init(const ArgsT &args) {
        return std::make_unique<PairIntInt>(args);
    }
    constexpr bool is_recursive() const override { return false; };
};

TEST_F(ClosureTest, TestTupleCaching) {
    LazyT<FnT<Int, Int>> fib = make_lazy<FnT<Int, Int>>(
        std::make_shared<TypedClosureG<WeakFnT<Int, Int>, Int, Int>>(
            FibFn::init));
    std::dynamic_pointer_cast<TypedClosureG<WeakFnT<Int, Int>, Int, Int>>(
        fib->lvalue())
        ->env = store_env<typename FibFn::EnvT>(fib);
    auto fib_fn = fib->value()->init(0);

    FnT<TupleT<Int, Int>, Int, Int> pair_fn =
        std::make_shared<TypedClosureG<Empty, TupleT<Int, Int>, Int, Int>>(
            PairIntInt::init);
    auto pair_5_6 = fib_fn->fn_call(pair_fn, Int{5}, Int{6});
    auto pair_5_7 = fib_fn->fn_call(pair_fn, Int{5}, Int{7});

    ASSERT_NE(pair_5_6, pair_5_7);
}
