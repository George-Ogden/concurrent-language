#pragma once

#include "fn/fn_gen.tpp"
#include "fn/fn_inst.tpp"
#include "fn/operators.hpp"
#include "fn/types.hpp"
#include "lazy/lazy.tpp"
#include "system/work_manager.tpp"
#include "types/builtin.hpp"
#include "types/compound.tpp"
#include "types/utils.hpp"
#include "work/runner.tpp"
#include "work/work.tpp"

#include <gtest/gtest.h>

#include <bit>
#include <memory>
#include <type_traits>
#include <utility>
#include <vector>

class FnCorrectnessTest : public ::testing::TestWithParam<unsigned> {
  protected:
    void SetUp() override {
        auto num_cpus = GetParam();
        ThreadManager::override_concurrency(num_cpus);
    }

    void TearDown() override { ThreadManager::reset_concurrency_override(); }
};

class IdentityInFn : public TypedClosureI<Empty, Int, Int> {
    using TypedClosureI<Empty, Int, Int>::TypedClosureI;
    LazyT<Int> body(LazyT<Int> &a) override { return a; }

  public:
    static std::unique_ptr<TypedFnI<Int, Int>> init(const ArgsT &args,
                                                    std::shared_ptr<void>) {
        return std::make_unique<IdentityInFn>(args);
    }
};

TEST_P(FnCorrectnessTest, IdentityTest) {
    FnT<Int, Int> identity_int{IdentityInFn::init};

    LazyT<Int> x = make_lazy<Int>(5);
    LazyT<Int> y = WorkManager::run(identity_int, x);
    ASSERT_EQ(y->value(), 5);
}

struct FourWayPlusV1 : public TypedClosureI<Empty, Int, Int, Int, Int, Int> {
    using TypedClosureI<Empty, Int, Int, Int, Int, Int>::TypedClosureI;
    LazyT<Int> res1 = nullptr, res2 = nullptr, res3 = nullptr;
    LazyT<Int> body(LazyT<Int> &a, LazyT<Int> &b, LazyT<Int> &c,
                    LazyT<Int> &d) {
        if (res1 == decltype(res1){}) {
            WorkT work;
            std::tie(work, res1) = Work::fn_call(Plus__BuiltIn_G, a, b);
            WorkManager::enqueue(work);
        }
        if (res2 == decltype(res2){}) {
            WorkT work;
            std::tie(work, res2) = Work::fn_call(Plus__BuiltIn_G, c, d);
            WorkManager::enqueue(work);
        }
        if (res3 == decltype(res3){}) {
            WorkT work;
            std::tie(work, res3) = Work::fn_call(Plus__BuiltIn_G, res1, res2);
            WorkManager::enqueue(work);
        }
        return res3;
    }
    static std::unique_ptr<TypedFnI<Int, Int, Int, Int, Int>>
    init(const ArgsT &args, std::shared_ptr<void>) {
        return std::make_unique<FourWayPlusV1>(args);
    }
};

TEST_P(FnCorrectnessTest, FourWayPlusV1Test) {
    FnT<Int, Int, Int, Int, Int> plus_fn{FourWayPlusV1::init};
    Int w = 11, x = 5, y = 10, z = 22;
    auto res = WorkManager::run(plus_fn, make_lazy<Int>(w), make_lazy<Int>(x),
                                make_lazy<Int>(y), make_lazy<Int>(z));
    ASSERT_EQ(res->value(), 48);
}

struct DelayedIncrement : public TypedClosureI<Empty, Int, Int> {
    using TypedClosureI<Empty, Int, Int>::TypedClosureI;
    LazyT<Int> res = nullptr;
    static inline bool finish;
    LazyT<Int> body(LazyT<Int> &x) {
        if (res == decltype(res){}) {
            WorkT work;
            std::tie(work, res) = Work::fn_call(Increment__BuiltIn_G, x);
            WorkManager::enqueue(work);
        }
        if (finish) {
            return res;
        } else {
            throw stack_inversion{};
        }
    }

    static std::unique_ptr<TypedFnI<Int, Int>> init(const ArgsT &args,
                                                    std::shared_ptr<void>) {
        return std::make_unique<DelayedIncrement>(args);
    }
};
TEST_P(FnCorrectnessTest, PersistenceTest) {
    ThreadManager::register_self(0);
    WorkRunner::shared_work_queue->clear();

    DelayedIncrement::finish = false;
    FnT<Int, Int> delayed{DelayedIncrement::init};
    auto [work, res] = Work::fn_call(delayed, make_lazy<Int>(7));
    EXPECT_THROW({ work->run(); }, stack_inversion);
    ASSERT_FALSE(res->done());
    DelayedIncrement::finish = true;
    work->status.cancel_work();
    work->run();
    ASSERT_EQ(WorkRunner::shared_work_queue->size(), 1);
    auto internal_work = WorkRunner::shared_work_queue->front().lock();
    ASSERT_NE(internal_work, nullptr);
    internal_work->run();
    ASSERT_TRUE(res->done());
    ASSERT_EQ(res->value(), 8);
}

struct BranchingExample : public TypedClosureI<Empty, Int, Int, Int, Int> {
    using TypedClosureI<Empty, Int, Int, Int, Int>::TypedClosureI;
    LazyT<Bool> res1;
    LazyT<Int> res2, res3;
    LazyT<Int> body(LazyT<Int> &x, LazyT<Int> &y, LazyT<Int> &z) override {
        WorkT call1, call2, call3;
        {
            std::tie(call1, res1) =
                Work::fn_call(Comparison_GE__BuiltIn_G, x, make_lazy<Int>(0));
            WorkManager::enqueue(call1);
        };
        WorkManager::await(res1);
        if (res1->value()) {
            std::tie(call2, res2) =
                Work::fn_call(Plus__BuiltIn_G, y, make_lazy<Int>(1));
            WorkManager::enqueue(call2);
        } else {
            std::tie(call2, res2) =
                Work::fn_call(Plus__BuiltIn_G, z, make_lazy<Int>(1));
            WorkManager::enqueue(call2);
        }
        std::tie(call3, res3) =
            Work::fn_call(Minus__BuiltIn_G, res2, make_lazy<Int>(2));
        WorkManager::enqueue(call3);
        return res3;
    }
    static std::unique_ptr<TypedFnI<Int, Int, Int, Int>>
    init(const ArgsT &args, std::shared_ptr<void>) {
        return std::make_unique<BranchingExample>(args);
    }
};

TEST_P(FnCorrectnessTest, PositiveBranchingExampleTest) {
    Int x = 5, y = 10, z = 22;
    FnT<Int, Int, Int, Int> branching_fn{BranchingExample::init};

    auto res = WorkManager::run(branching_fn, make_lazy<Int>(x),
                                make_lazy<Int>(y), make_lazy<Int>(z));

    ASSERT_EQ(res->value(), 9);
}

TEST_P(FnCorrectnessTest, NegativeBranchingExampleTest) {
    Int x = -5, y = 10, z = 22;
    FnT<Int, Int, Int, Int> branching_fn{BranchingExample::init};

    auto res = WorkManager::run(branching_fn, make_lazy<Int>(x),
                                make_lazy<Int>(y), make_lazy<Int>(z));

    ASSERT_EQ(res->value(), 21);
}

struct HigherOrderCall : public TypedClosureI<Empty, Int, FnT<Int, Int>, Int> {
    using TypedClosureI<Empty, Int, FnT<Int, Int>, Int>::TypedClosureI;
    LazyT<Int> res;
    LazyT<Int> body(LazyT<FnT<Int, Int>> &f, LazyT<Int> &x) override {
        WorkManager::await(f);
        WorkT call;
        std::tie(call, res) = Work::fn_call(f->value(), x);
        WorkManager::enqueue(call);
        return res;
    }
    static std::unique_ptr<TypedFnI<Int, FnT<Int, Int>, Int>>
    init(const ArgsT &args, std::shared_ptr<void>) {
        return std::make_unique<HigherOrderCall>(args);
    }
};

TEST_P(FnCorrectnessTest, HigherOrderFnExampleTest) {
    LazyT<FnT<Int, Int>> decrement =
        make_lazy<FnT<Int, Int>>(Decrement__BuiltIn_G);
    Int x = 5;
    FnT<Int, FnT<Int, Int>, Int> higher_order_call_fn{HigherOrderCall::init};
    auto res =
        WorkManager::run(higher_order_call_fn, decrement, make_lazy<Int>(x));
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
            std::tie(call1, res1) =
                Work::fn_call(FnT<Int, Int>{RecursiveDouble::init}, arg);
            WorkManager::enqueue(call1);
            std::tie(call2, res2) =
                Work::fn_call(Plus__BuiltIn_G, res1, make_lazy<Int>(2));
            WorkManager::enqueue(call2);
            return res2;
        } else {
            return make_lazy<Int>(0);
        }
    }
    static std::unique_ptr<TypedFnI<Int, Int>> init(const ArgsT &args,
                                                    std::shared_ptr<void>) {
        return std::make_unique<RecursiveDouble>(args);
    }
};

TEST_P(FnCorrectnessTest, RecursiveDoubleTest1) {
    Int x = 5;
    FnT<Int, Int> recursive_double_fn{RecursiveDouble::init};
    auto res = WorkManager::run(recursive_double_fn, make_lazy<Int>(x));
    ASSERT_EQ(res->value(), 10);
}

TEST_P(FnCorrectnessTest, RecursiveDoubleTest2) {
    Int x = -5;
    FnT<Int, Int> recursive_double_fn{RecursiveDouble::init};
    auto res = WorkManager::run(recursive_double_fn, make_lazy<Int>(x));
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
        std::tie(work, z) = Work::fn_call(Negation__BuiltIn_G, y);
        return std::make_tuple(x, std::make_tuple(z));
    }
    static std::unique_ptr<TypedFnI<TupleT<Int, TupleT<Bool>>, Int, Bool>>
    init(const ArgsT &args, std::shared_ptr<void>) {
        return std::make_unique<PairIntBool>(args);
    }
};

TEST_P(FnCorrectnessTest, TupleTest) {
    Int x = 5;
    Bool y = true;

    LazyT<FnT<TupleT<Int, TupleT<Bool>>, Int, Bool>> pair_fn;
    pair_fn = make_lazy<remove_lazy_t<decltype(pair_fn)>>(PairIntBool::init);
    auto res = WorkManager::run(pair_fn->value(), make_lazy<Int>(x),
                                make_lazy<Bool>(y));
    ASSERT_EQ(std::get<0>(res)->value(), 5);
    ASSERT_EQ(std::get<0>(std::get<1>(res))->value(), false);
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
    static std::unique_ptr<TypedFnI<Bool, Bull>> init(const ArgsT &args,
                                                      std::shared_ptr<void>) {
        return std::make_unique<BoolUnion>(args);
    }
};

TEST_P(FnCorrectnessTest, ValueFreeUnionTest) {
    FnT<Bool, Bull> bool_union_fn{BoolUnion::init};
    {
        Bull bull{};
        bull.tag = 0ULL;
        auto res = WorkManager::run(bool_union_fn, make_lazy<Bull>(bull));
        ASSERT_TRUE(res->value());
    }

    {
        Bull bull{};
        bull.tag = 1ULL;
        auto res = WorkManager::run(bool_union_fn, make_lazy<Bull>(bull));
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
        switch (x.tag) {
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
    static std::unique_ptr<TypedFnI<Bool, EitherIntBool>>
    init(const ArgsT &args, std::shared_ptr<void>) {
        return std::make_unique<EitherIntBoolFn>(args);
    }
};

TEST_P(FnCorrectnessTest, ValueIncludedUnionTest) {
    FnT<Bool, EitherIntBool> either_int_bool_fn{EitherIntBoolFn::init};
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
            new (&either.value) Right{make_lazy<Bool>(value)};
        }

        auto res = WorkManager::run(either_int_bool_fn,
                                    make_lazy<EitherIntBool>(either));
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
            y = Comparison_GT__BuiltIn(i, z);
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
    static std::unique_ptr<TypedFnI<Bool, EitherIntBool>>
    init(const ArgsT &args, std::shared_ptr<void>) {
        return std::make_unique<EitherIntBoolEdgeCaseFn>(args);
    }
};

TEST_P(FnCorrectnessTest, EdgeCaseTest) {
    FnT<Bool, EitherIntBool> either_int_bool_fn{EitherIntBoolEdgeCaseFn::init};
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

        auto res = WorkManager::run(either_int_bool_fn,
                                    make_lazy<EitherIntBool>(either));
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

            auto [call1, res1] =
                Work::fn_call(FnT<Int, ListInt>{ListIntSum::init}, tail);
            WorkManager::enqueue(call1);

            auto [call2, res2] = Work::fn_call(Plus__BuiltIn_G, res1, head);
            WorkManager::enqueue(call2);
            return res2;
        }
        case 1:
            return make_lazy<Int>(0);
        }
        return nullptr;
    }
    static std::unique_ptr<TypedFnI<Int, ListInt>> init(const ArgsT &args,
                                                        std::shared_ptr<void>) {
        return std::make_unique<ListIntSum>(args);
    }
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

    FnT<Int, ListInt> summer{ListIntSum::init};
    auto res = WorkManager::run(summer, first);
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

            auto [call, res] =
                Work::fn_call(FnT<ListInt, ListInt>{ListIntDec::init}, tail);
            WorkManager::enqueue(call);

            return make_lazy<ListInt>(
                std::integral_constant<std::size_t, 0>(),
                Cons{std::make_tuple(Decrement__BuiltIn(head), res)});
        }
        case 1:
            return make_lazy<ListInt>(std::integral_constant<std::size_t, 1>(),
                                      Nil{});
        }
        return nullptr;
    }
    static std::unique_ptr<TypedFnI<ListInt, ListInt>>
    init(const ArgsT &args, std::shared_ptr<void>) {
        return std::make_unique<ListIntDec>(args);
    }
};

TEST_P(FnCorrectnessTest, RecursiveTypeTest2) {
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

    FnT<ListInt, ListInt> summer{ListIntDec::init};
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
    static std::unique_ptr<TypedFnI<Nat, Nat>> init(const ArgsT &args,
                                                    std::shared_ptr<void>) {
        return std::make_unique<PredFn>(args);
    }
};

TEST_P(FnCorrectnessTest, SimpleRecursiveTypeTest) {
    LazyT<Nat> n = make_lazy<Nat>(std::integral_constant<std::size_t, 1>());
    LazyT<Nat> inner =
        make_lazy<Nat>(std::integral_constant<std::size_t, 0>(), Suc{n});
    LazyT<Nat> outer =
        make_lazy<Nat>(std::integral_constant<std::size_t, 0>(), Suc{inner});

    FnT<Nat, Nat> pred_fn{PredFn::init};

    auto res = WorkManager::run(pred_fn, outer)->value();

    ASSERT_EQ(res.tag, inner->value().tag);
    auto tmp = inner->value().value;
    ASSERT_EQ(reinterpret_cast<Suc *>(&res.value)->value,
              reinterpret_cast<Suc *>(&tmp)->value);
}

struct RecursiveFn : public TypedClosureI<TupleT<FnT<Int, Int>>, Int, Int> {
    using TypedClosureI<TupleT<FnT<Int, Int>>, Int, Int>::TypedClosureI;
    LazyT<Int> res;
    LazyT<Int> body(LazyT<Int> &x) override {
        WorkManager::await(x);
        if (x->value() > 0) {
            auto arg = Decrement__BuiltIn(x);
            WorkT work;
            LazyT<FnT<Int, Int>> call_fn = std::get<0>(env);
            std::tie(work, res) = Work::fn_call(call_fn->value(), arg);
            WorkManager::enqueue(work);
            return res;
        } else {
            return x;
        }
    }
    static std::unique_ptr<TypedFnI<Int, Int>> init(const ArgsT &args,
                                                    std::shared_ptr<EnvT> env) {
        return std::make_unique<RecursiveFn>(args, *env);
    }
};

TEST_P(FnCorrectnessTest, SelfRecursiveFnTest) {
    LazyT<FnT<Int, Int>> fn;
    fn = make_lazy<remove_lazy_t<decltype(fn)>>(
        ClosureFnT<remove_lazy_t<typename RecursiveFn::EnvT>,
                   remove_lazy_t<decltype(fn)>>(RecursiveFn::init));
    LazyT<TupleT<FnT<Int, Int>>> env =
        std::make_tuple(make_lazy<FnT<Int, Int>>(fn->value()));
    std::bit_cast<ClosureFnT<remove_lazy_t<typename RecursiveFn::EnvT>,
                             remove_lazy_t<decltype(fn)>> *>(&fn->lvalue())
        ->env() = env;
    LazyT<Int> x = make_lazy<Int>(5);

    auto res = WorkManager::run(fn->value(), x);
    ASSERT_EQ(res->value(), 0);
}

struct IsEven : public TypedClosureI<TupleT<FnT<Bool, Int>>, Bool, Int> {
    using TypedClosureI<TupleT<FnT<Bool, Int>>, Bool, Int>::TypedClosureI;
    LazyT<Bool> body(LazyT<Int> &x) override {
        WorkManager::await(x);
        if (x->value() > 0) {
            auto y = Decrement__BuiltIn(x);
            auto [call, res] = Work::fn_call(std::get<0>(env)->value(), y);
            WorkManager::enqueue(call);
            return res;
        } else {
            return make_lazy<Bool>(true);
        }
    }
    static std::unique_ptr<TypedFnI<Bool, Int>>
    init(const ArgsT &args, std::shared_ptr<EnvT> env) {
        return std::make_unique<IsEven>(args, *env);
    }
};

struct IsOdd : public TypedClosureI<TupleT<FnT<Bool, Int>>, Bool, Int> {
    using TypedClosureI<TupleT<FnT<Bool, Int>>, Bool, Int>::TypedClosureI;
    LazyT<Bool> body(LazyT<Int> &x) override {
        WorkManager::await(x);
        if (x->value() > 0) {
            auto y = Decrement__BuiltIn(x);
            auto [call, res] = Work::fn_call(std::get<0>(env)->value(), y);
            WorkManager::enqueue(call);
            return res;
        } else {
            return make_lazy<Bool>(false);
        }
    }
    static std::unique_ptr<TypedFnI<Bool, Int>>
    init(const ArgsT &args, std::shared_ptr<EnvT> env) {
        return std::make_unique<IsOdd>(args, *env);
    }
};

TEST_P(FnCorrectnessTest, MutuallyRecursiveFnsTest) {
    LazyT<FnT<Bool, Int>> is_even_fn;
    LazyT<FnT<Bool, Int>> is_odd_fn;

    is_even_fn = make_lazy<remove_lazy_t<decltype(is_even_fn)>>(
        ClosureFnT<LazyT<typename IsEven::EnvT>,
                   remove_lazy_t<decltype(is_even_fn)>>(IsEven::init));
    is_odd_fn = make_lazy<remove_lazy_t<decltype(is_odd_fn)>>(
        ClosureFnT<LazyT<typename IsOdd::EnvT>,
                   remove_lazy_t<decltype(is_odd_fn)>>(IsOdd::init));

    LazyT<TupleT<FnT<Bool, Int>>> is_odd_env = std::make_tuple(is_even_fn);
    LazyT<TupleT<FnT<Bool, Int>>> is_even_env = std::make_tuple(is_odd_fn);

    std::bit_cast<ClosureFnT<LazyT<typename IsEven::EnvT>,
                             remove_lazy_t<decltype(is_even_fn)>> *>(
        &is_even_fn->lvalue())
        ->env() = is_even_env;
    std::bit_cast<ClosureFnT<LazyT<typename IsOdd::EnvT>,
                             remove_lazy_t<decltype(is_odd_fn)>> *>(
        &is_odd_fn->lvalue())
        ->env() = is_odd_env;

    for (auto x : {5, 10, 23, 0}) {
        auto even = WorkManager::run(is_even_fn->value(), make_lazy<Int>(x));
        ASSERT_EQ(even->value(), x % 2 == 0);
        auto odd = WorkManager::run(is_odd_fn->value(), make_lazy<Int>(x));
        ASSERT_EQ(odd->value(), x % 2 == 1);
    }
}

std::vector<unsigned> cpu_counts = {1, 2, 3, 4};
INSTANTIATE_TEST_SUITE_P(FnCorrectnessTests, FnCorrectnessTest,
                         ::testing::ValuesIn(cpu_counts));
