#pragma once

#include "data_structures/lazy.tpp"

#include <gtest/gtest.h>

#include <atomic>
#include <memory>

class LazyConstantTest : public ::testing::Test {
  protected:
    LazyT<Int> x = std::make_shared<Lazy<Int>>(3);
    void SetUp() override { ThreadManager::register_self(0); }
};

TEST_F(LazyConstantTest, AlwaysDone) { ASSERT_TRUE(x->done()); }

TEST_F(LazyConstantTest, CorrectValue) { ASSERT_EQ(x->value(), 3); }

TEST(MakeLazyTest, CorrectValue) {
    LazyT<Int> y = make_lazy<Int>(-3);
    ASSERT_EQ(y->value(), -3);
}
