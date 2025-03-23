#pragma once

#include "fn/fn_gen.tpp"
#include "fn/fn_inst.tpp"
#include "fn/operators.hpp"
#include "fn/types.hpp"
#include "lazy/fns.hpp"
#include "lazy/lazy.tpp"
#include "system/work_manager.tpp"
#include "types/builtin.hpp"
#include "types/compound.tpp"
#include "types/utils.hpp"
#include "work/runner.tpp"
#include "work/work.tpp"

#include <gtest/gtest.h>

#include <memory>
#include <type_traits>
#include <utility>
#include <vector>

class FnCorrectnessTest : public ::testing::TestWithParam<unsigned> {
  protected:
    void SetUp() override {
        auto num_cpus = GetParam();
        ThreadManager::override_concurrency(num_cpus);
        ThreadManager::register_self(0);
    }

    void TearDown() override { ThreadManager::reset_concurrency_override(); }
};

struct IdentityInt : TypedClosureI<Empty, Int, Int> {
    using TypedClosureI<Empty, Int, Int>::TypedClosureI;
    LazyT<Int> body(LazyT<Int> &x) override { return x; }

  public:
    constexpr std::size_t lower_size_bound() const override { return 1; };
    constexpr std::size_t upper_size_bound() const override { return 1; };
    static std::unique_ptr<TypedFnI<Int, Int>> init(const ArgsT &args) {
        return std::make_unique<IdentityInt>(args);
    }
    constexpr bool is_recursive() const override { return false; };
};

TEST_P(FnCorrectnessTest, IdentityTest) {
    FnT<Int, Int> identity_int =
        std::make_shared<TypedClosureG<Empty, Int, Int>>(IdentityInt::init);

    LazyT<Int> y = WorkManager::run(identity_int, Int{5});
    ASSERT_EQ(y->value(), 5);
}

struct FourWayPlus : TypedClosureI<Empty, Int, Int, Int, Int, Int> {
    using TypedClosureI<Empty, Int, Int, Int, Int, Int>::TypedClosureI;
    LazyT<Int> body(LazyT<Int> &a, LazyT<Int> &b, LazyT<Int> &c,
                    LazyT<Int> &d) override {
        auto res1 = Plus__BuiltIn(a, b);
        auto res2 = Plus__BuiltIn(c, d);
        auto res3 = Plus__BuiltIn(res1, res2);
        return ensure_lazy(res3);
    }
    constexpr std::size_t lower_size_bound() const override { return 50; };
    constexpr std::size_t upper_size_bound() const override { return 50; };
    static std::unique_ptr<TypedFnI<Int, Int, Int, Int, Int>>
    init(const ArgsT &args) {
        return std::make_unique<FourWayPlus>(args);
    }
    constexpr bool is_recursive() const override { return false; };
};

TEST_P(FnCorrectnessTest, FourWayPlusTest) {
    FnT<Int, Int, Int, Int, Int> plus_fn =
        std::make_shared<TypedClosureG<Empty, Int, Int, Int, Int, Int>>(
            FourWayPlus::init);
    Int w = 11, x = 5, y = 10, z = 22;
    auto res = WorkManager::run(plus_fn, w, x, y, z);
    ASSERT_EQ(res->value(), 48);
}

struct DelayedIncrement : public TypedClosureI<Empty, Int, Int> {
    using TypedClosureI<Empty, Int, Int>::TypedClosureI;
    LazyT<Int> res = nullptr;
    static inline bool finish;
    LazyT<Int> body(LazyT<Int> &x) override {
        res = fn_call(Increment__BuiltIn_G, x);
        if (finish) {
            return res;
        } else {
            throw stack_inversion{};
        }
    }
    constexpr std::size_t lower_size_bound() const override { return 10; };
    constexpr std::size_t upper_size_bound() const override { return 80; };

    static std::unique_ptr<TypedFnI<Int, Int>> init(const ArgsT &args) {
        return std::make_unique<DelayedIncrement>(args);
    }
    constexpr bool is_recursive() const override { return false; };
};

struct BranchingExample : public TypedClosureI<Empty, Int, Int, Int, Int> {
    using TypedClosureI<Empty, Int, Int, Int, Int>::TypedClosureI;
    LazyT<Bool> res1;
    LazyT<Int> res2, res3;
    LazyT<Int> body(LazyT<Int> &x, LazyT<Int> &y, LazyT<Int> &z) override {
        { res1 = fn_call(Comparison_GE__BuiltIn_G, x, Int(0)); };
        WorkManager::await(res1);
        if (res1->value()) {
            res2 = fn_call(Plus__BuiltIn_G, y, Int(1));
        } else {
            res2 = fn_call(Plus__BuiltIn_G, z, Int(1));
        }
        res3 = fn_call(Minus__BuiltIn_G, res2, Int(2));
        return res3;
    }
    constexpr std::size_t lower_size_bound() const override { return 100; };
    constexpr std::size_t upper_size_bound() const override { return 100; };
    static std::unique_ptr<TypedFnI<Int, Int, Int, Int>>
    init(const ArgsT &args) {
        return std::make_unique<BranchingExample>(args);
    }
    constexpr bool is_recursive() const override { return false; };
};

TEST_P(FnCorrectnessTest, PositiveBranchingExampleTest) {
    Int x = 5, y = 10, z = 22;
    FnT<Int, Int, Int, Int> branching_fn =
        std::make_shared<TypedClosureG<Empty, Int, Int, Int, Int>>(
            BranchingExample::init);

    auto res = WorkManager::run(branching_fn, Int{x}, Int{y}, Int{z});

    ASSERT_EQ(res->value(), 9);
}

TEST_P(FnCorrectnessTest, NegativeBranchingExampleTest) {
    Int x = -5, y = 10, z = 22;
    FnT<Int, Int, Int, Int> branching_fn =
        std::make_shared<TypedClosureG<Empty, Int, Int, Int, Int>>(
            BranchingExample::init);

    auto res = WorkManager::run(branching_fn, Int{x}, Int{y}, Int{z});

    ASSERT_EQ(res->value(), 21);
}

struct HigherOrderCall : public TypedClosureI<Empty, Int, FnT<Int, Int>, Int> {
    using TypedClosureI<Empty, Int, FnT<Int, Int>, Int>::TypedClosureI;
    LazyT<Int> res;
    LazyT<Int> body(LazyT<FnT<Int, Int>> &f, LazyT<Int> &x) override {
        WorkManager::await(f);
        WorkT call;
        res = fn_call(f->value(), x);
        return res;
    }
    constexpr std::size_t lower_size_bound() const override { return 60; };
    constexpr std::size_t upper_size_bound() const override { return 60; };
    static std::unique_ptr<TypedFnI<Int, FnT<Int, Int>, Int>>
    init(const ArgsT &args) {
        return std::make_unique<HigherOrderCall>(args);
    }
    constexpr bool is_recursive() const override { return false; };
};

TEST_P(FnCorrectnessTest, HigherOrderFnExampleTest) {
    FnT<Int, Int> decrement = Decrement__BuiltIn_G;
    Int x = 5;
    FnT<Int, FnT<Int, Int>, Int> higher_order_call_fn =
        std::make_shared<TypedClosureG<Empty, Int, FnT<Int, Int>, Int>>(
            HigherOrderCall::init);
    auto res = WorkManager::run(higher_order_call_fn, decrement, x);
    ASSERT_EQ(res->value(), 4);
}

struct RecursiveDouble : public TypedClosureI<Empty, Int, Int> {
    using TypedClosureI<Empty, Int, Int>::TypedClosureI;
    LazyT<Int> res1, res2;
    LazyT<Int> body(LazyT<Int> &x) override {
        WorkManager::await(x);
        if (x->value() > 0) {
            auto arg = Decrement__BuiltIn(x);
            WorkT call1, call2;
            FnT<Int, Int> fn = std::make_shared<TypedClosureG<Empty, Int, Int>>(
                RecursiveDouble::init);
            res1 = fn_call(fn, arg);
            res2 = fn_call(Plus__BuiltIn_G, res1, make_lazy<Int>(2));
            return res2;
        } else {
            return make_lazy<Int>(0);
        }
    }
    constexpr std::size_t lower_size_bound() const override { return 10; };
    constexpr std::size_t upper_size_bound() const override { return 150; };
    static std::unique_ptr<TypedFnI<Int, Int>> init(const ArgsT &args) {
        return std::make_unique<RecursiveDouble>(args);
    }
    constexpr bool is_recursive() const override { return true; };
};

TEST_P(FnCorrectnessTest, RecursiveDoubleTest1) {
    Int x = 5;
    FnT<Int, Int> recursive_double_fn =
        std::make_shared<TypedClosureG<Empty, Int, Int>>(RecursiveDouble::init);
    auto res = WorkManager::run(recursive_double_fn, x);
    ASSERT_EQ(res->value(), 10);
}

TEST_P(FnCorrectnessTest, RecursiveDoubleTest2) {
    Int x = -5;
    FnT<Int, Int> recursive_double_fn =
        std::make_shared<TypedClosureG<Empty, Int, Int>>(RecursiveDouble::init);
    auto res = WorkManager::run(recursive_double_fn, x);
    ASSERT_EQ(res->value(), 0);
}

struct PairIntBool
    : public TypedClosureI<Empty, TupleT<Int, TupleT<Bool>>, Int, Bool> {
    using TypedClosureI<Empty, TupleT<Int, TupleT<Bool>>, Int,
                        Bool>::TypedClosureI;
    LazyT<Bool> z;
    LazyT<TupleT<Int, TupleT<Bool>>> body(LazyT<Int> &x,
                                          LazyT<Bool> &y) override {
        WorkT work;
        z = fn_call(Negation__BuiltIn_G, y);
        return std::make_tuple(x, std::make_tuple(z));
    }
    constexpr std::size_t lower_size_bound() const override { return 60; };
    constexpr std::size_t upper_size_bound() const override { return 60; };
    static std::unique_ptr<TypedFnI<TupleT<Int, TupleT<Bool>>, Int, Bool>>
    init(const ArgsT &args) {
        return std::make_unique<PairIntBool>(args);
    }
    static inline FnT<TupleT<Int, TupleT<Bool>>, Int, Bool> G =
        std::make_shared<
            TypedClosureG<Empty, TupleT<Int, TupleT<Bool>>, Int, Bool>>(init);
    constexpr bool is_recursive() const override { return false; };
};

TEST_P(FnCorrectnessTest, TupleTest) {
    Int x = 5;
    Bool y = true;

    LazyT<FnT<TupleT<Int, TupleT<Bool>>, Int, Bool>> pair_fn;
    pair_fn = make_lazy<remove_lazy_t<decltype(pair_fn)>>(PairIntBool::G);
    auto res = WorkManager::run(pair_fn->value(), x, y);
    ASSERT_EQ(std::get<0>(res)->value(), 5);
    ASSERT_EQ(std::get<0>(std::get<1>(res))->value(), false);
}

class MultiplyFn : public TypedClosureI<TupleT<Int>, Int, Int> {
    using TypedClosureI<TupleT<Int>, Int, Int>::TypedClosureI;
    LazyT<Int> body(LazyT<Int> &a) override {
        auto b = std::get<0>(env);
        auto c = Multiply__BuiltIn(a, b);
        return ensure_lazy(c);
    }

    constexpr bool is_recursive() const override { return false; };

  public:
    constexpr std::size_t lower_size_bound() const override { return 60; };
    constexpr std::size_t upper_size_bound() const override { return 60; };
    static std::unique_ptr<TypedFnI<Int, Int>> init(const ArgsT &args,
                                                    const EnvT &env) {
        return std::make_unique<MultiplyFn>(args, env);
    }
};

TEST_P(FnCorrectnessTest, MultiplierTest) {
    LazyT<FnT<Int, Int>> fn;
    fn = setup_closure<MultiplyFn>();
    auto env = std::make_tuple(Int{10});
    std::dynamic_pointer_cast<
        ClosureFnT<remove_lazy_t<typename MultiplyFn::EnvT>,
                   remove_shared_ptr_t<remove_lazy_t<decltype(fn)>>>>(
        fn->lvalue())
        ->env = store_env<typename MultiplyFn::EnvT>(env);

    auto res = WorkManager::run(fn->value(), Int{5});
    ASSERT_EQ(res->value(), 50);
}

struct Twoo;
struct Faws;
typedef VariantT<Twoo, Faws> Bull;
struct Twoo {};
struct Faws {};

struct BoolUnion : public TypedClosureI<Empty, Bool, Bull> {
    using TypedClosureI<Empty, Bool, Bull>::TypedClosureI;
    LazyT<Bool> body(LazyT<Bull> &x) override {
        WorkManager::await(x);
        return make_lazy<Bool>(x->value().tag == 0);
    }
    constexpr std::size_t lower_size_bound() const override { return 20; };
    constexpr std::size_t upper_size_bound() const override { return 20; };
    static std::unique_ptr<TypedFnI<Bool, Bull>> init(const ArgsT &args) {
        return std::make_unique<BoolUnion>(args);
    }
    constexpr bool is_recursive() const override { return false; };
};

TEST_P(FnCorrectnessTest, ValueFreeUnionTest) {
    FnT<Bool, Bull> bool_union_fn =
        std::make_shared<TypedClosureG<Empty, Bool, Bull>>(BoolUnion::init);
    {
        Bull bull{};
        bull.tag = 0ULL;
        auto res = WorkManager::run(bool_union_fn, bull);
        ASSERT_TRUE(res->value());
    }

    {
        Bull bull{};
        bull.tag = 1ULL;
        auto res = WorkManager::run(bool_union_fn, bull);
        ASSERT_FALSE(res->value());
    }
}

struct Left;
struct Right;
typedef VariantT<Left, Right> EitherIntBool;
struct Left {
    using type = LazyT<Int>;
    type value;
};
struct Right {
    using type = LazyT<Bool>;
    type value;
};

struct EitherIntBoolFn : public TypedClosureI<Empty, Bool, EitherIntBool> {
    using TypedClosureI<Empty, Bool, EitherIntBool>::TypedClosureI;
    LazyT<Bool> body(LazyT<EitherIntBool> &either) override {
        WorkManager::await(either);
        EitherIntBool x = either->value();
        switch (extract_lazy(either).tag) {
        case 0ULL: {
            auto left = reinterpret_cast<Left *>(&x.value)->value;
            WorkManager::await(left);
            return make_lazy<Bool>(left->value() > 10);
        }
        case 1ULL: {
            auto right = reinterpret_cast<Right *>(&x.value)->value;
            WorkManager::await(right);
            return right;
        }
        }
        return 0;
    }
    constexpr std::size_t lower_size_bound() const override { return 50; };
    constexpr std::size_t upper_size_bound() const override { return 50; };
    static std::unique_ptr<TypedFnI<Bool, EitherIntBool>>
    init(const ArgsT &args) {
        return std::make_unique<EitherIntBoolFn>(args);
    }
    constexpr bool is_recursive() const override { return false; };
};

TEST_P(FnCorrectnessTest, ValueIncludedUnionTest) {
    FnT<Bool, EitherIntBool> either_int_bool_fn =
        std::make_shared<TypedClosureG<Empty, Bool, EitherIntBool>>(
            EitherIntBoolFn::init);
    for (const auto &[tag, value, result] :
         std::vector<std::tuple<int, int, bool>>{{1, 0, false},
                                                 {1, 1, true},
                                                 {0, 0, false},
                                                 {0, 5, false},
                                                 {0, 15, true}}) {
        EitherIntBool either{};
        either.tag = tag;
        if (tag == 0) {
            new (&either.value) Left{make_lazy<Int>(value)};
        } else {
            new (&either.value) Right{make_lazy<Bool>(value > 0)};
        }

        auto res = WorkManager::run(either_int_bool_fn, either);
        ASSERT_EQ(res->value(), result);
    }
}

struct EitherIntBoolEdgeCaseFn
    : public TypedClosureI<Empty, Bool, EitherIntBool> {
    using TypedClosureI<Empty, Bool, EitherIntBool>::TypedClosureI;
    LazyT<Bool> body(LazyT<EitherIntBool> &either) override {
        WorkManager::await(either);
        EitherIntBool x = either->value();
        LazyT<Bool> y;
        switch (x.tag) {
        case 0ULL: {
            LazyT<Left::type> i = reinterpret_cast<Left *>(&x.value)->value;
            LazyT<Int> z = make_lazy<Int>(0);
            y = ensure_lazy(Comparison_GT__BuiltIn(i, z));
            break;
        }
        case 1ULL: {
            LazyT<Right::type> b = reinterpret_cast<Right *>(&x.value)->value;
            y = b;
            break;
        }
        }
        return y;
    }
    constexpr std::size_t lower_size_bound() const override { return 30; };
    constexpr std::size_t upper_size_bound() const override { return 90; };
    static std::unique_ptr<TypedFnI<Bool, EitherIntBool>>
    init(const ArgsT &args) {
        return std::make_unique<EitherIntBoolEdgeCaseFn>(args);
    }
    constexpr bool is_recursive() const override { return false; };
};

TEST_P(FnCorrectnessTest, EdgeCaseTest) {
    FnT<Bool, EitherIntBool> either_int_bool_fn =
        std::make_shared<TypedClosureG<Empty, Bool, EitherIntBool>>(
            EitherIntBoolEdgeCaseFn::init);
    for (const auto &[tag, value, result] :
         std::vector<std::tuple<int, int, bool>>{{1, 0, false},
                                                 {1, 1, true},
                                                 {0, 0, false},
                                                 {0, -5, false},
                                                 {0, 15, true}}) {
        EitherIntBool either{};
        either.tag = tag;
        if (tag == 0) {

            new (&either.value) Left{make_lazy<Int>(value)};
        } else {
            new (&either.value) Right{make_lazy<Bool>(value)};
        }

        auto res = WorkManager::run(either_int_bool_fn, either);
        ASSERT_EQ(res->value(), result);
    }
}

struct Cons;
struct Nil;
typedef VariantT<Cons, Nil> ListInt;
struct Cons {
    using type = TupleT<Int, ListInt>;
    LazyT<type> value;
};
struct Nil {
    Empty value;
};

struct ListIntSum : public TypedClosureI<Empty, Int, ListInt> {
    using TypedClosureI<Empty, Int, ListInt>::TypedClosureI;
    LazyT<Int> body(LazyT<ListInt> &lazy_list) override {
        WorkManager::await(lazy_list);
        ListInt list = lazy_list->value();
        switch (list.tag) {
        case 0: {
            LazyT<Cons::type> cons =
                reinterpret_cast<Cons *>(&list.value)->value;
            WorkManager::await(cons);
            LazyT<Int> head = std::get<0ULL>(cons);
            LazyT<ListInt> tail = std::get<1ULL>(cons);

            FnT<Int, ListInt> fn =
                std::make_shared<TypedClosureG<Empty, Int, ListInt>>(
                    ListIntSum::init);
            auto res1 = fn_call(fn, tail);
            auto res2 = fn_call(Plus__BuiltIn_G, res1, head);
            return ensure_lazy(res2);
        }
        case 1:
            return make_lazy<Int>(0);
        }
        return nullptr;
    }
    constexpr std::size_t lower_size_bound() const override { return 20; };
    constexpr std::size_t upper_size_bound() const override { return 200; };
    static std::unique_ptr<TypedFnI<Int, ListInt>> init(const ArgsT &args) {
        return std::make_unique<ListIntSum>(args);
    }
    constexpr bool is_recursive() const override { return true; };
};

TEST_P(FnCorrectnessTest, RecursiveTypeTest1) {
    LazyT<ListInt> tail;
    tail = make_lazy<remove_lazy_t<decltype(tail)>>(
        std::integral_constant<std::size_t, 1>(), Nil{});
    LazyT<ListInt> third;
    third = make_lazy<remove_lazy_t<decltype(third)>>(
        std::integral_constant<std::size_t, 0>(),
        Cons{std::make_tuple(make_lazy<Int>(8), tail)});
    LazyT<ListInt> second;
    second = make_lazy<remove_lazy_t<decltype(second)>>(
        std::integral_constant<std::size_t, 0>(),
        Cons{std::make_tuple(make_lazy<Int>(4), third)});
    LazyT<ListInt> first;
    first = make_lazy<remove_lazy_t<decltype(first)>>(
        std::integral_constant<std::size_t, 0>(),
        Cons{std::make_tuple(make_lazy<Int>(-9), second)});

    FnT<Int, ListInt> summer =
        std::make_shared<TypedClosureG<Empty, Int, ListInt>>(ListIntSum::init);
    auto res = WorkManager::run(summer, first->value());
    ASSERT_EQ(res->value(), 3);
}

struct ListIntDec : public TypedClosureI<Empty, ListInt, ListInt> {
    using TypedClosureI<Empty, ListInt, ListInt>::TypedClosureI;
    LazyT<ListInt> body(LazyT<ListInt> &lazy_list) override {
        WorkManager::await(lazy_list);
        ListInt list = lazy_list->value();
        switch (list.tag) {
        case 0: {
            LazyT<Cons::type> cons =
                reinterpret_cast<Cons *>(&list.value)->value;
            WorkManager::await(cons);
            LazyT<Int> head = std::get<0ULL>(cons);
            LazyT<ListInt> tail = std::get<1ULL>(cons);

            FnT<ListInt, ListInt> fn =
                std::make_shared<TypedClosureG<Empty, ListInt, ListInt>>(
                    ListIntDec::init);
            auto res = fn_call(fn, tail);

            return make_lazy<ListInt>(std::integral_constant<std::size_t, 0>(),
                                      Cons{ensure_lazy(std::make_tuple(
                                          Decrement__BuiltIn(head), res))});
        }
        case 1:
            return make_lazy<ListInt>(std::integral_constant<std::size_t, 1>(),
                                      Nil{});
        }
        return nullptr;
    }
    constexpr std::size_t lower_size_bound() const override { return 30; };
    constexpr std::size_t upper_size_bound() const override { return 200; };
    static std::unique_ptr<TypedFnI<ListInt, ListInt>> init(const ArgsT &args) {
        return std::make_unique<ListIntDec>(args);
    }
    constexpr bool is_recursive() const override { return true; };
};

TEST_P(FnCorrectnessTest, RecursiveTypeTest2) {
    auto tail = ListInt{std::integral_constant<std::size_t, 1>(), Nil{}};
    auto third =
        ListInt{std::integral_constant<std::size_t, 0>(),
                Cons{ensure_lazy(std::make_tuple(make_lazy<Int>(8), tail))}};
    auto second =
        ListInt{std::integral_constant<std::size_t, 0>(),
                Cons{ensure_lazy(std::make_tuple(make_lazy<Int>(4), third))}};
    auto first =
        ListInt{std::integral_constant<std::size_t, 0>(),
                Cons{ensure_lazy(std::make_tuple(make_lazy<Int>(-9), second))}};

    FnT<ListInt, ListInt> summer =
        std::make_shared<TypedClosureG<Empty, ListInt, ListInt>>(
            ListIntDec::init);
    auto res = WorkManager::run(summer, first);
    ASSERT_TRUE(res->done());
    ASSERT_EQ(res->value().tag, 0);
    auto body = reinterpret_cast<Cons *>(&res->lvalue().value)->value;
    ASSERT_EQ(std::get<0>(body)->value(), -10);

    auto next = std::get<1>(body);
    ASSERT_TRUE(next->done());
    ASSERT_EQ(next->value().tag, 0);
    body = reinterpret_cast<Cons *>(&next->lvalue().value)->value;
    ASSERT_EQ(std::get<0>(body)->value(), 3);

    next = std::get<1>(body);
    ASSERT_TRUE(next->done());
    ASSERT_EQ(next->value().tag, 0);
    body = reinterpret_cast<Cons *>(&next->lvalue().value)->value;
    ASSERT_EQ(std::get<0>(body)->value(), 7);

    next = std::get<1>(body);
    ASSERT_TRUE(next->done());
    ASSERT_EQ(next->value().tag, 1);
}

struct Suc;
typedef VariantT<Suc, Nil> Nat;
struct Suc {
    using type = Nat;
    LazyT<type> value;
};

struct PredFn : public TypedClosureI<Empty, Nat, Nat> {
    using TypedClosureI<Empty, Nat, Nat>::TypedClosureI;
    LazyT<Nat> body(LazyT<Nat> &nat) override {
        WorkManager::await(nat);
        Nat nat_ = nat->value();
        switch (nat_.tag) {
        case 0: {
            LazyT<Suc::type> s = reinterpret_cast<Suc *>(&nat_.value)->value;
            return s;
        }
        case 1: {
            return make_lazy<Nat>(std::integral_constant<std::size_t, 1>(),
                                  Nil{});
        }
        }
        return nullptr;
    }
    constexpr std::size_t lower_size_bound() const override { return 20; };
    constexpr std::size_t upper_size_bound() const override { return 40; };
    static std::unique_ptr<TypedFnI<Nat, Nat>> init(const ArgsT &args) {
        return std::make_unique<PredFn>(args);
    }
    constexpr bool is_recursive() const override { return true; };
};

TEST_P(FnCorrectnessTest, SimpleRecursiveTypeTest) {
    auto n = Nat{std::integral_constant<std::size_t, 1>()};
    auto inner =
        Nat{std::integral_constant<std::size_t, 0>(), Suc{ensure_lazy(n)}};
    auto outer =
        Nat{std::integral_constant<std::size_t, 0>(), Suc{ensure_lazy(inner)}};

    FnT<Nat, Nat> pred_fn =
        std::make_shared<TypedClosureG<Empty, Nat, Nat>>(PredFn::init);

    auto res = WorkManager::run(pred_fn, outer)->value();

    ASSERT_EQ(res.tag, inner.tag);
    ASSERT_EQ(reinterpret_cast<Suc *>(&res.value)->value,
              reinterpret_cast<Suc *>(&inner.value)->value);
}

struct RecursiveFn : public TypedClosureI<TupleT<WeakFnT<Int, Int>>, Int, Int> {
    using TypedClosureI<TupleT<WeakFnT<Int, Int>>, Int, Int>::TypedClosureI;
    LazyT<Int> res;
    LazyT<Int> body(LazyT<Int> &x) override {
        auto y = Comparison_GT__BuiltIn(x, Int{0});
        WorkManager::await(y);
        if (extract_lazy(y)) {
            auto arg = Decrement__BuiltIn(x);
            WorkT work;
            LazyT<FnT<Int, Int>> call_fn = load_env(std::get<0>(env));
            res = fn_call(call_fn->value(), arg);
            return res;
        } else {
            return x;
        }
    }
    constexpr std::size_t lower_size_bound() const override { return 5; };
    constexpr std::size_t upper_size_bound() const override { return 100; };
    static std::unique_ptr<TypedFnI<Int, Int>> init(const ArgsT &args,
                                                    const EnvT &env) {
        return std::make_unique<RecursiveFn>(args, env);
    }
    constexpr bool is_recursive() const override { return true; };
};

TEST_P(FnCorrectnessTest, SelfRecursiveFnTest) {
    LazyT<FnT<Int, Int>> fn;
    fn = setup_closure<RecursiveFn>();
    LazyT<TupleT<FnT<Int, Int>>> env =
        std::make_tuple(make_lazy<FnT<Int, Int>>(fn->value()));
    std::dynamic_pointer_cast<
        ClosureFnT<remove_lazy_t<typename RecursiveFn::EnvT>,
                   remove_shared_ptr_t<remove_lazy_t<decltype(fn)>>>>(
        fn->lvalue())
        ->env = store_env<typename RecursiveFn::EnvT>(env);

    auto move = [](auto &x) {
        auto tmp = x;
        x = nullptr;
        return tmp;
    };

    auto res = WorkManager::run(move(fn)->value(), Int{5});
    ASSERT_EQ(fn, nullptr);
    ASSERT_EQ(res->value(), 0);
}

struct IsEven : public TypedClosureI<TupleT<WeakFnT<Bool, Int>>, Bool, Int> {
    using TypedClosureI<TupleT<WeakFnT<Bool, Int>>, Bool, Int>::TypedClosureI;
    LazyT<Bool> body(LazyT<Int> &x) override {
        auto c = Comparison_GT__BuiltIn(x, Int{0});
        WorkManager::await(c);
        if (extract_lazy(c)) {
            auto y = Decrement__BuiltIn(x);
            auto res = fn_call(load_env(std::get<0>(env))->value(), y);
            return res;
        } else {
            return make_lazy<Bool>(true);
        }
    }
    constexpr std::size_t lower_size_bound() const override { return 10; };
    constexpr std::size_t upper_size_bound() const override { return 120; };
    static std::unique_ptr<TypedFnI<Bool, Int>> init(const ArgsT &args,
                                                     const EnvT &env) {
        return std::make_unique<IsEven>(args, env);
    }
    static inline FnT<Bool, Int> G =
        std::make_shared<TypedClosureG<TupleT<WeakFnT<Bool, Int>>, Bool, Int>>(
            init);
    constexpr bool is_recursive() const override { return true; };
};

struct IsOdd : public TypedClosureI<TupleT<WeakFnT<Bool, Int>>, Bool, Int> {
    using TypedClosureI<TupleT<WeakFnT<Bool, Int>>, Bool, Int>::TypedClosureI;
    LazyT<Bool> body(LazyT<Int> &x) override {
        auto c = Comparison_GT__BuiltIn(x, Int{0});
        WorkManager::await(c);
        if (extract_lazy(c)) {
            auto y = Decrement__BuiltIn(x);
            auto res = fn_call(load_env(std::get<0>(env))->value(), y);
            return res;
        } else {
            return make_lazy<Bool>(false);
        }
    }
    constexpr std::size_t lower_size_bound() const override { return 10; };
    constexpr std::size_t upper_size_bound() const override { return 120; };
    static std::unique_ptr<TypedFnI<Bool, Int>> init(const ArgsT &args,
                                                     const EnvT &env) {
        return std::make_unique<IsOdd>(args, env);
    }
    static inline FnT<Bool, Int> G =
        std::make_shared<TypedClosureG<TupleT<WeakFnT<Bool, Int>>, Bool, Int>>(
            init);
    constexpr bool is_recursive() const override { return true; };
};

TEST_P(FnCorrectnessTest, MutuallyRecursiveFnsAllocatorTest) {

    for (auto x : {5, 10, 23, 0}) {
        LazyT<FnT<Bool, Int>> is_odd_fn;

        {
            LazyT<FnT<Bool, Int>> is_even_fn;

            struct Allocator {
                ClosureFnT<remove_lazy_t<typename IsOdd::EnvT>,
                           typename IsOdd::Fn>
                    _0;
                ClosureFnT<remove_lazy_t<typename IsEven::EnvT>,
                           typename IsEven::Fn>
                    _1;
            };
            std::shared_ptr<Allocator> allocator =
                std::make_shared<Allocator>();

            is_odd_fn = setup_closure<IsOdd>(allocator, allocator->_0);
            is_even_fn = setup_closure<IsEven>(allocator, allocator->_1);

            auto is_odd_env = std::make_tuple(is_even_fn);
            auto is_even_env = std::make_tuple(is_odd_fn);

            std::dynamic_pointer_cast<ClosureFnT<
                remove_lazy_t<typename IsEven::EnvT>,
                remove_shared_ptr_t<remove_lazy_t<decltype(is_even_fn)>>>>(
                is_even_fn->lvalue())
                ->env = store_env<typename IsEven::EnvT>(is_even_env);
            std::dynamic_pointer_cast<ClosureFnT<
                remove_lazy_t<typename IsOdd::EnvT>,
                remove_shared_ptr_t<remove_lazy_t<decltype(is_odd_fn)>>>>(
                is_odd_fn->lvalue())
                ->env = store_env<typename IsOdd::EnvT>(is_odd_env);
        }

        auto odd = WorkManager::run(is_odd_fn->value(), Int{x});
        ASSERT_EQ(odd->value(), x % 2 == 1);
    }
}

std::vector<unsigned> cpu_counts = {1, 2, 3, 4};
INSTANTIATE_TEST_SUITE_P(FnCorrectnessTests, FnCorrectnessTest,
                         ::testing::ValuesIn(cpu_counts));
