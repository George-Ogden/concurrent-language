#pragma once

#include "data_structures/lazy.tpp"
#include "fn/fn.tpp"
#include "fn/operators.hpp"
#include "system/work_manager.tpp"
#include "types/builtin.hpp"
#include "types/compound.tpp"
#include "types/utils.hpp"

#include <gtest/gtest.h>

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

const std::vector<unsigned> cpu_counts = {1, 2, 3, 4};
INSTANTIATE_TEST_SUITE_P(FnCorrectnessTests, FnCorrectnessTest,
                         ::testing::ValuesIn(cpu_counts));
