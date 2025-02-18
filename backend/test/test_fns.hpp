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
    std::shared_ptr<Work> call2;
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

std::vector<unsigned> cpu_counts = {1, 2, 3, 4};
INSTANTIATE_TEST_SUITE_P(FnCorrectnessTests, FnCorrectnessTest,
                         ::testing::ValuesIn(cpu_counts));
