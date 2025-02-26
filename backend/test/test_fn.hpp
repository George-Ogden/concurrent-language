#pragma once

#include "fn/fn_gen.tpp"
#include "fn/fn_inst.tpp"
#include "lazy/fns.hpp"
#include "lazy/lazy.tpp"
#include "types/builtin.hpp"

#include <gtest/gtest.h>

#include <bit>
#include <memory>

class PlusFn : public TypedClosureI<Empty, Int, Int, Int> {
    using TypedClosureI<Empty, Int, Int, Int>::TypedClosureI;
    LazyT<Int> body(LazyT<Int> &a, LazyT<Int> &b) override {
        return make_lazy<Int>(a->value() + b->value());
    }

  public:
    static std::unique_ptr<TypedFnI<Int, Int, Int>>
    init(const ArgsT &args, std::shared_ptr<void>) {
        return std::make_unique<PlusFn>(args);
    }
};

TEST(FnTest, TestFnCall) {
    TypedFnG<Int, Int, Int> plus_fn{PlusFn::init};
    ASSERT_EQ(
        plus_fn.init(make_lazy<Int>(3), make_lazy<Int>(4))->run()->value(), 7);
}

class ClosureTest : public ::testing::Test {
  protected:
    void SetUp() override {
        ThreadManager::override_concurrency(1);
        ThreadManager::register_self(0);
        WorkRunner::shared_work_queue->clear();
        WorkManager::runners.clear();
        WorkManager::runners.emplace_back(std::make_unique<WorkRunner>(0));
    }

    void TearDown() override { ThreadManager::reset_concurrency_override(); }
};

class AdderFn : public TypedClosureI<Int, Int, Int> {
    using TypedClosureI<Int, Int, Int>::TypedClosureI;
    LazyT<Int> body(LazyT<Int> &a) override {
        return make_lazy<Int>(a->value() + load_env(env)->value());
    }

  public:
    static std::unique_ptr<TypedFnI<Int, Int>> init(const ArgsT &args,
                                                    std::shared_ptr<EnvT> env) {
        return std::make_unique<AdderFn>(args, *env);
    }
};

TEST_F(ClosureTest, TestClosureCall) {
    LazyT<Int> env = make_lazy<Int>(7);
    TypedClosureG<Int, Int, Int> adder_fn{AdderFn::init, env};
    ASSERT_EQ(adder_fn.init(make_lazy<Int>(4))->run()->value(), 11);
}

LazyT<Int> call_closure(TypedFnG<Int, Int> &f, LazyT<Int> a) {
    return f.init(a)->run();
}

TEST_F(ClosureTest, TestFnCast) {
    LazyT<Int> env = make_lazy<Int>(4);
    TypedClosureG<Int, Int, Int> adder_fn{AdderFn::init, env};
    ASSERT_EQ(call_closure(adder_fn, make_lazy<Int>(7))->value(), 11);
}

class FibFn : public TypedClosureI<WeakTypedFnG<Int, Int>, Int, Int> {
    using TypedClosureI<WeakTypedFnG<Int, Int>, Int, Int>::TypedClosureI;
    LazyT<Int> body(LazyT<Int> &x) override {
        Int y = x->value();
        if (y < 0) {
            return make_lazy<Int>(0);
        } else if (y <= 1) {
            return make_lazy<Int>(1);
        } else {
            LazyT<TypedFnG<Int, Int>> f = load_env(env);
            return make_lazy<Int>(
                f->value().init(make_lazy<Int>(y - 1))->run()->value() +
                f->value().init(make_lazy<Int>(y - 2))->run()->value());
        }
    }

  public:
    static std::unique_ptr<TypedFnI<Int, Int>> init(const ArgsT &args,
                                                    std::shared_ptr<EnvT> env) {
        return std::make_unique<FibFn>(args, *env);
    }
};

TEST_F(ClosureTest, TestRecursiveClosure) {
    LazyT<TypedFnG<Int, Int>> foo_fn = make_lazy<TypedFnG<Int, Int>>(
        TypedClosureG<WeakTypedFnG<Int, Int>, Int, Int>(FibFn::init));
    std::bit_cast<TypedClosureG<WeakTypedFnG<Int, Int>, Int, Int> *>(
        &foo_fn->lvalue())
        ->env() = store_env<typename FibFn::EnvT>(foo_fn);

    ASSERT_EQ(call_closure(foo_fn->lvalue(), make_lazy<Int>(5))->value(), 8);
}
