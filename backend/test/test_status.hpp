#pragma once

#include "system/thread_manager.tpp"
#include "work/status.hpp"

#include <gtest/gtest.h>

TEST(AtomicSharedEnumTest, BitFlip) {
    AtomicSharedEnum<1, 2, 1> byte_array;
    ASSERT_EQ(byte_array.get<0>(), 0);
    ASSERT_EQ(byte_array.get<1>(), 0);
    ASSERT_EQ(byte_array.get<2>(), 0);
    ASSERT_FALSE(byte_array.flip<0>());
    ASSERT_EQ(byte_array.get<0>(), 1);
    ASSERT_EQ(byte_array.get<1>(), 0);
    ASSERT_EQ(byte_array.get<2>(), 0);
    ASSERT_FALSE(byte_array.flip<2>());
    ASSERT_EQ(byte_array.get<0>(), 1);
    ASSERT_EQ(byte_array.get<1>(), 0);
    ASSERT_EQ(byte_array.get<2>(), 1);
    ASSERT_TRUE(byte_array.flip<2>());
    ASSERT_EQ(byte_array.get<0>(), 1);
    ASSERT_EQ(byte_array.get<1>(), 0);
    ASSERT_EQ(byte_array.get<2>(), 0);
}

TEST(AtomicSharedEnumTest, CompareExchange) {
    AtomicSharedEnum<2, 1> byte_array;
    ASSERT_EQ(byte_array.get<0>(), 0);
    ASSERT_EQ(byte_array.get<1>(), 0);
    ASSERT_TRUE(byte_array.compare_exchange<0>(0, 3));
    ASSERT_EQ(byte_array.get<0>(), 3);
    ASSERT_EQ(byte_array.get<1>(), 0);
    ASSERT_FALSE(byte_array.compare_exchange<0>(2, 1));
    ASSERT_EQ(byte_array.get<0>(), 3);
    ASSERT_EQ(byte_array.get<1>(), 0);
    ASSERT_TRUE(byte_array.compare_exchange<1>(0, 1));
    ASSERT_EQ(byte_array.get<0>(), 3);
    ASSERT_EQ(byte_array.get<1>(), 1);
    ASSERT_FALSE(byte_array.compare_exchange<1>(0, 1));
    ASSERT_EQ(byte_array.get<0>(), 3);
    ASSERT_EQ(byte_array.get<1>(), 1);
    ASSERT_TRUE(byte_array.compare_exchange<0>(3, 2));
    ASSERT_EQ(byte_array.get<0>(), 2);
    ASSERT_EQ(byte_array.get<1>(), 1);
    ASSERT_TRUE(byte_array.compare_exchange<1>(1, 0));
    ASSERT_EQ(byte_array.get<0>(), 2);
    ASSERT_EQ(byte_array.get<1>(), 0);
}

TEST(AtomicSharedEnumTest, Exchange) {
    AtomicSharedEnum<2, 1> byte_array;
    ASSERT_EQ(byte_array.get<0>(), 0);
    ASSERT_EQ(byte_array.get<1>(), 0);
    ASSERT_EQ(byte_array.exchange<0>(3), 0);
    ASSERT_EQ(byte_array.get<0>(), 3);
    ASSERT_EQ(byte_array.get<1>(), 0);
    ASSERT_EQ(byte_array.exchange<0>(1), 3);
    ASSERT_EQ(byte_array.get<0>(), 1);
    ASSERT_EQ(byte_array.get<1>(), 0);
    ASSERT_EQ(byte_array.exchange<1>(1), 0);
    ASSERT_EQ(byte_array.get<0>(), 1);
    ASSERT_EQ(byte_array.get<1>(), 1);
    ASSERT_EQ(byte_array.exchange<1>(1), 1);
    ASSERT_EQ(byte_array.get<0>(), 1);
    ASSERT_EQ(byte_array.get<1>(), 1);
    ASSERT_EQ(byte_array.exchange<0>(2), 1);
    ASSERT_EQ(byte_array.get<0>(), 2);
    ASSERT_EQ(byte_array.get<1>(), 1);
    ASSERT_EQ(byte_array.exchange<1>(0), 1);
    ASSERT_EQ(byte_array.get<0>(), 2);
    ASSERT_EQ(byte_array.get<1>(), 0);
}
