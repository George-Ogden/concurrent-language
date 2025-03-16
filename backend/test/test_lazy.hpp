#pragma once

#include "lazy/fns.hpp"
#include "lazy/lazy.tpp"
#include "work/work.tpp"

#include <gtest/gtest.h>

#include <atomic>
#include <memory>
#include <optional>

class LazyConstantTest : public ::testing::Test {
  protected:
    LazyT<Int> x = make_lazy<Int>(3);
    void SetUp() override { ThreadManager::register_self(0); }
};

TEST_F(LazyConstantTest, AlwaysDone) { ASSERT_TRUE(x->done()); }

TEST_F(LazyConstantTest, CorrectValue) { ASSERT_EQ(x->value(), 3); }

TEST(MakeLazyTest, CorrectValue) {
    LazyT<Int> y = make_lazy<Int>(-3);
    ASSERT_EQ(y->value(), -3);
}

TEST(EnsureLazyTest, NonLazy) {
    LazyT<Int> y = ensure_lazy(Int{-3});
    ASSERT_EQ(y->value(), -3);
}

TEST(EnsureLazyTest, Lazy) {
    LazyT<Int> y = ensure_lazy(make_lazy<Int>(-3));
    ASSERT_EQ(y->value(), -3);
}

TEST(EnsureLazyTest, MixedTuple) {
    auto y =
        ensure_lazy(std::make_tuple(3, std::make_tuple(make_lazy<Int>(-3))));
    ASSERT_EQ(std::get<0>(y)->value(), 3);
    ASSERT_EQ(std::get<0>(std::get<1>(y))->value(), -3);
}

TEST(ExtractLazyTest, NonLazy) {
    Int y = extract_lazy(Int{-3});
    ASSERT_EQ(y, -3);
}

TEST(ExtractLazyTest, Lazy) {
    Int y = extract_lazy(make_lazy<Int>(-3));
    ASSERT_EQ(y, -3);
}

TEST(ExtractLazyTest, MixedTuple) {
    auto y =
        extract_lazy(std::make_tuple(3, std::make_tuple(make_lazy<Int>(-3))));
    ASSERT_EQ(std::get<0>(y), 3);
    ASSERT_EQ(std::get<0>(std::get<1>(y)), -3);
}

TEST(LazyCacheTest, BooleanCache) {
    LazyT<Bool> t0 = make_lazy<Bool>(true);
    LazyT<Bool> t1 = make_lazy<Bool>(true);
    LazyT<Bool> f0 = make_lazy<Bool>(false);
    LazyT<Bool> f1 = make_lazy<Bool>(false);
    ASSERT_TRUE(t0->value());
    ASSERT_TRUE(t1->value());
    ASSERT_FALSE(f0->value());
    ASSERT_FALSE(f1->value());

    ASSERT_EQ(t0.get(), t1.get());
    ASSERT_EQ(f0.get(), f1.get());
}

TEST(LazyCacheTest, IntegerCache) {
    for (Int i = -128; i < 128; i++) {
        LazyT<Int> m = make_lazy<Int>(i);
        LazyT<Int> n = make_lazy<Int>(i);
        ASSERT_EQ(m->value(), i);
        ASSERT_EQ(n->value(), i);
        ASSERT_EQ(m.get(), n.get());
    }
}

TEST(LazyCacheTest, IntegerCacheBounds) {
    for (Int i : std::vector<Int>{-129, 128}) {
        LazyT<Int> m = make_lazy<Int>(i);
        LazyT<Int> n = make_lazy<Int>(i);
        ASSERT_EQ(m->value(), i);
        ASSERT_EQ(n->value(), i);
        ASSERT_NE(m.get(), n.get());
    }
}
