#pragma once

#include "fn/fn.tpp"
#include "fn/operators.hpp"
#include "lazy/lazy.tpp"
#include "system/work_manager.tpp"
#include "types/builtin.hpp"
#include "types/compound.tpp"
#include "types/utils.hpp"

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
    }

    void TearDown() override { ThreadManager::reset_concurrency_override(); }
};

LazyT<Int> identity_int(LazyT<Int> x, std::shared_ptr<void>) { return x; }

TEST_P(FnCorrectnessTest, IdentityTest) {
    FnT<Int, Int> identity_int_fn{identity_int};

    LazyT<Int> x = make_lazy<Int>(5);
    LazyT<Int> y = WorkManager::run(identity_int_fn, x);
    ASSERT_EQ(y->value(), 5);
}

LazyT<Int> four_way_plus_v1(LazyT<Int> a, LazyT<Int> b, LazyT<Int> c,
                            LazyT<Int> d, std::shared_ptr<void>) {
    auto [call1, res1] = Work::fn_call(Plus__BuiltIn_Fn, a, b);
    WorkManager::enqueue(call1);
    auto [call2, res2] = Work::fn_call(Plus__BuiltIn_Fn, c, d);
    WorkManager::enqueue(call2);
    auto [call3, res3] = Work::fn_call(Plus__BuiltIn_Fn, res1, res2);
    WorkManager::enqueue(call3);
    return res3;
};

LazyT<Int> four_way_plus_v2(LazyT<Int> a, LazyT<Int> b, LazyT<Int> c,
                            LazyT<Int> d, std::shared_ptr<void>) {
    auto [call1, res1] = Work::fn_call(Plus__BuiltIn_Fn, a, b);
    WorkManager::enqueue(call1);
    auto [call2, res2] = Work::fn_call(Plus__BuiltIn_Fn, res1, c);
    WorkManager::enqueue(call2);
    auto [call3, res3] = Work::fn_call(Plus__BuiltIn_Fn, res2, d);
    WorkManager::enqueue(call3);
    return res3;
};

TEST_P(FnCorrectnessTest, FourWayPlusV1Test) {
    FnT<Int, Int, Int, Int, Int> four_way_plus_v1_fn{four_way_plus_v1};
    Int w = 11, x = 5, y = 10, z = 22;
    auto res = WorkManager::run(four_way_plus_v1_fn, make_lazy<Int>(w),
                                make_lazy<Int>(x), make_lazy<Int>(y),
                                make_lazy<Int>(z));
    ASSERT_EQ(res->value(), 48);
}

TEST_P(FnCorrectnessTest, FourWayPlusV2Test) {
    FnT<Int, Int, Int, Int, Int> four_way_plus_v2_fn{four_way_plus_v2};
    Int w = 11, x = 5, y = 10, z = 22;
    auto res = WorkManager::run(four_way_plus_v2_fn, make_lazy<Int>(w),
                                make_lazy<Int>(x), make_lazy<Int>(y),
                                make_lazy<Int>(z));
    ASSERT_EQ(res->value(), 48);
}

LazyT<Int> branching_example(LazyT<Int> x, LazyT<Int> y, LazyT<Int> z,
                             std::shared_ptr<void> env = nullptr) {
    auto [call1, res1] =
        Work::fn_call(Comparison_GE__BuiltIn_Fn, x, make_lazy<Int>(0));
    WorkManager::enqueue(call1);
    WorkManager::await(res1);
    WorkT call2;
    LazyT<Int> res2;
    if (res1->value()) {
        std::tie(call2, res2) =
            Work::fn_call(Plus__BuiltIn_Fn, y, make_lazy<Int>(1));
        WorkManager::enqueue(call2);
    } else {
        std::tie(call2, res2) =
            Work::fn_call(Plus__BuiltIn_Fn, z, make_lazy<Int>(1));
        WorkManager::enqueue(call2);
    }
    auto [call3, res3] =
        Work::fn_call(Minus__BuiltIn_Fn, res2, make_lazy<Int>(2));
    WorkManager::enqueue(call3);
    return res3;
}

TEST_P(FnCorrectnessTest, PositiveBranchingExampleTest) {
    Int x = 5, y = 10, z = 22;
    FnT<Int, Int, Int, Int> branching_fn{branching_example};

    auto res = WorkManager::run(branching_fn, make_lazy<Int>(x),
                                make_lazy<Int>(y), make_lazy<Int>(z));

    ASSERT_EQ(res->value(), 9);
}

TEST_P(FnCorrectnessTest, NegativeBranchingExampleTest) {
    Int x = -5, y = 10, z = 22;
    FnT<Int, Int, Int, Int> branching_fn{branching_example};

    auto res = WorkManager::run(branching_fn, make_lazy<Int>(x),
                                make_lazy<Int>(y), make_lazy<Int>(z));

    ASSERT_EQ(res->value(), 21);
}

LazyT<Int> adder(LazyT<Int> x, std::shared_ptr<LazyT<TupleT<Int>>> env) {
    LazyT<Int> y = std::get<0>(*env);
    auto [call, res] = Work::fn_call(Plus__BuiltIn_Fn, x, y);
    WorkManager::enqueue(call);
    return res;
}

LazyT<Int> higher_order_call(LazyT<FnT<Int, Int>> f, LazyT<Int> x,
                             std::shared_ptr<void> env = nullptr) {
    WorkManager::await(f);
    auto [call, res] = Work::fn_call(f->value(), x);
    WorkManager::enqueue(call);
    return res;
}

TEST_P(FnCorrectnessTest, HigherOrderFnExampleTest) {
    LazyT<TupleT<Int>> env = std::make_tuple(make_lazy<Int>(4));
    ClosureT<TupleT<Int>, Int, Int> adder_closure{adder,
                                                  LazyT<TupleT<Int>>(env)};
    LazyT<FnT<Int, Int>> adder_fn = make_lazy<FnT<Int, Int>>(adder_closure);
    Int x = 5;
    FnT<Int, FnT<Int, Int>, Int> higher_order_call_fn{higher_order_call};
    auto res =
        WorkManager::run(higher_order_call_fn, adder_fn, make_lazy<Int>(x));
    ASSERT_EQ(res->value(), 9);
}

LazyT<Int> recursive_double(LazyT<Int> x, std::shared_ptr<void> env = nullptr) {
    WorkManager::await(x);
    if (x->value() > 0) {
        auto arg = Decrement__BuiltIn(x);
        auto [call1, res1] =
            Work::fn_call(FnT<Int, Int>{recursive_double}, arg);
        WorkManager::enqueue(call1);

        auto [extra_call, _] =
            Work::fn_call(FnT<Int, Int>{recursive_double}, arg);
        WorkManager::enqueue(extra_call);

        auto [call2, res2] =
            Work::fn_call(Plus__BuiltIn_Fn, res1, make_lazy<Int>(2));
        WorkManager::enqueue(call2);
        return res2;
    } else {
        return std::make_shared<LazyConstant<Int>>(0);
    }
}

TEST_P(FnCorrectnessTest, RecursiveDoubleTest1) {
    Int x = 5;
    FnT<Int, Int> recursive_double_fn{recursive_double};
    auto res = WorkManager::run(recursive_double_fn, make_lazy<Int>(x));
    ASSERT_EQ(res->value(), 10);
}

TEST_P(FnCorrectnessTest, RecursiveDoubleTest2) {
    Int x = -5;
    FnT<Int, Int> recursive_double_fn{recursive_double};
    auto res = WorkManager::run(recursive_double_fn, make_lazy<Int>(x));
    ASSERT_EQ(res->value(), 0);
}

LazyT<TupleT<Int, TupleT<Bool>>>
pair_int_bool(LazyT<Int> x, LazyT<Bool> y,
              std::shared_ptr<void> env = nullptr) {
    return std::make_tuple(x, std::make_tuple(y));
}

TEST_P(FnCorrectnessTest, TupleTest) {
    Int x = 5;
    Bool y = true;

    FnT<TupleT<Int, TupleT<Bool>>, Int, Bool> pair_fn{pair_int_bool};
    auto res = WorkManager::run(pair_fn, make_lazy<Int>(x), make_lazy<Bool>(y));
    ASSERT_EQ(std::get<0>(res)->value(), 5);
    ASSERT_EQ(std::get<0>(std::get<1>(res))->value(), true);
}

struct Twoo;
struct Faws;
typedef VariantT<Twoo, Faws> Bull;
struct Twoo {};
struct Faws {};

LazyT<Bool> bool_union(LazyT<Bull> x, std::shared_ptr<void> env = nullptr) {
    WorkManager::await(x);
    return std::make_shared<LazyConstant<Bool>>(x->value().tag == 0);
}

TEST_P(FnCorrectnessTest, ValueFreeUnionTest) {
    FnT<Bool, Bull> bool_union_fn{bool_union};
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

LazyT<Bool> either_int_bool(LazyT<EitherIntBool> either,
                            std::shared_ptr<void> env = nullptr) {
    WorkManager::await(either);
    EitherIntBool x = either->value();
    switch (x.tag) {
    case 0ULL: {
        auto left = reinterpret_cast<Left *>(&x.value)->value;
        WorkManager::await(left);
        return std::make_shared<LazyConstant<Bool>>(left->value() > 10);
    }
    case 1ULL: {
        auto right = reinterpret_cast<Right *>(&x.value)->value;
        WorkManager::await(right);
        return std::make_shared<LazyConstant<Bool>>(right->value());
    }
    }
    return 0;
}

TEST_P(FnCorrectnessTest, ValueIncludedUnionTest) {
    FnT<Bool, EitherIntBool> either_int_bool_fn{either_int_bool};
    for (const auto &[tag, value, result] :
         std::vector<std::tuple<int, int, bool>>{{1, 0, false},
                                                 {1, 1, true},
                                                 {0, 0, false},
                                                 {0, 5, false},
                                                 {0, 15, true}}) {
        EitherIntBool either{};
        either.tag = tag;
        if (tag == 0) {
            new (&either.value)
                Left{std::make_shared<LazyConstant<Int>>(value)};
        } else {
            new (&either.value)
                Right{std::make_shared<LazyConstant<Bool>>(value)};
        }

        auto res = WorkManager::run(either_int_bool_fn,
                                    make_lazy<EitherIntBool>(either));
        ASSERT_EQ(res->value(), result);
    }
}

LazyT<Bool> either_int_bool_edge_case(LazyT<EitherIntBool> either,
                                      std::shared_ptr<void> env = nullptr) {
    WorkManager::await(either);
    EitherIntBool x = either->value();
    LazyT<Bool> y;
    switch (x.tag) {
    case 0ULL: {
        LazyT<Left::type> i = reinterpret_cast<Left *>(&x.value)->value;
        LazyT<Int> z = std::make_shared<LazyConstant<Int>>(0);
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

TEST_P(FnCorrectnessTest, EdgeCaseTest) {
    FnT<Bool, EitherIntBool> either_int_bool_fn{either_int_bool_edge_case};
    for (const auto &[tag, value, result] :
         std::vector<std::tuple<int, int, bool>>{{1, 0, false},
                                                 {1, 1, true},
                                                 {0, 0, false},
                                                 {0, -5, false},
                                                 {0, 15, true}}) {
        EitherIntBool either{};
        either.tag = tag;
        if (tag == 0) {

            new (&either.value)
                Left{std::make_shared<LazyConstant<Int>>(value)};
        } else {
            new (&either.value)
                Right{std::make_shared<LazyConstant<Bool>>(value)};
        }

        auto res = WorkManager::run(either_int_bool_fn,
                                    make_lazy<EitherIntBool>(either));
        ASSERT_EQ(res->value(), result);
    }
}

std::vector<unsigned> cpu_counts = {1, 2, 3, 4};
INSTANTIATE_TEST_SUITE_P(FnCorrectnessTests, FnCorrectnessTest,
                         ::testing::ValuesIn(cpu_counts));
