#pragma once

#include "data_structures/atomic_shared_enum.hpp"
#include "system/thread_manager.tpp"

#include <gtest/gtest.h>

#include <array>
#include <type_traits>

TEST(PrefixSumTest, PrefixSum) {
    ASSERT_EQ((prefix_sum_v<1, 3, 2, 2>),
              (std::array<std::size_t, 5>{0, 1, 4, 6, 8}));
}

TEST(AtomicSharedEnumTest, BitFlip) {
    AtomicSharedEnum<1, 2, 1> byte_array;
    ASSERT_EQ(byte_array.load<0>(), 0);
    ASSERT_EQ(byte_array.load<1>(), 0);
    ASSERT_EQ(byte_array.load<2>(), 0);
    ASSERT_FALSE(byte_array.flip<0>());
    ASSERT_EQ(byte_array.load<0>(), 1);
    ASSERT_EQ(byte_array.load<1>(), 0);
    ASSERT_EQ(byte_array.load<2>(), 0);
    ASSERT_FALSE(byte_array.flip<2>());
    ASSERT_EQ(byte_array.load<0>(), 1);
    ASSERT_EQ(byte_array.load<1>(), 0);
    ASSERT_EQ(byte_array.load<2>(), 1);
    ASSERT_TRUE(byte_array.flip<2>());
    ASSERT_EQ(byte_array.load<0>(), 1);
    ASSERT_EQ(byte_array.load<1>(), 0);
    ASSERT_EQ(byte_array.load<2>(), 0);
}

TEST(AtomicSharedEnumTest, CompareExchange) {
    AtomicSharedEnum<2, 1, 2, 1> byte_array;
    ASSERT_EQ(byte_array.load<0>(), 0);
    ASSERT_EQ(byte_array.load<1>(), 0);
    ASSERT_EQ(byte_array.load<2>(), 0);
    ASSERT_EQ(byte_array.load<3>(), 0);
    ASSERT_TRUE(byte_array.compare_exchange<0>(0, 3));
    ASSERT_EQ(byte_array.load<0>(), 3);
    ASSERT_EQ(byte_array.load<1>(), 0);
    ASSERT_EQ(byte_array.load<2>(), 0);
    ASSERT_EQ(byte_array.load<3>(), 0);
    ASSERT_FALSE(byte_array.compare_exchange<0>(2, 1));
    ASSERT_EQ(byte_array.load<0>(), 3);
    ASSERT_EQ(byte_array.load<1>(), 0);
    ASSERT_EQ(byte_array.load<2>(), 0);
    ASSERT_EQ(byte_array.load<3>(), 0);
    ASSERT_TRUE(byte_array.compare_exchange<3>(0, 1));
    ASSERT_EQ(byte_array.load<0>(), 3);
    ASSERT_EQ(byte_array.load<1>(), 0);
    ASSERT_EQ(byte_array.load<2>(), 0);
    ASSERT_EQ(byte_array.load<3>(), 1);
    ASSERT_TRUE(byte_array.compare_exchange<1>(0, 1));
    ASSERT_EQ(byte_array.load<0>(), 3);
    ASSERT_EQ(byte_array.load<1>(), 1);
    ASSERT_EQ(byte_array.load<2>(), 0);
    ASSERT_EQ(byte_array.load<3>(), 1);
    ASSERT_FALSE(byte_array.compare_exchange<3>(0, 1));
    ASSERT_EQ(byte_array.load<0>(), 3);
    ASSERT_EQ(byte_array.load<1>(), 1);
    ASSERT_EQ(byte_array.load<2>(), 0);
    ASSERT_EQ(byte_array.load<3>(), 1);
    ASSERT_FALSE(byte_array.compare_exchange<0>(2, 1));
    ASSERT_EQ(byte_array.load<0>(), 3);
    ASSERT_EQ(byte_array.load<1>(), 1);
    ASSERT_EQ(byte_array.load<2>(), 0);
    ASSERT_EQ(byte_array.load<3>(), 1);
    ASSERT_TRUE(byte_array.compare_exchange<1>(1, 0));
    ASSERT_EQ(byte_array.load<0>(), 3);
    ASSERT_EQ(byte_array.load<1>(), 0);
    ASSERT_EQ(byte_array.load<2>(), 0);
    ASSERT_EQ(byte_array.load<3>(), 1);
    ASSERT_TRUE(byte_array.compare_exchange<0>(3, 1));
    ASSERT_EQ(byte_array.load<0>(), 1);
    ASSERT_EQ(byte_array.load<1>(), 0);
    ASSERT_EQ(byte_array.load<2>(), 0);
    ASSERT_EQ(byte_array.load<3>(), 1);
    ASSERT_TRUE(byte_array.compare_exchange<3>(1, 0));
    ASSERT_EQ(byte_array.load<0>(), 1);
    ASSERT_EQ(byte_array.load<1>(), 0);
    ASSERT_EQ(byte_array.load<2>(), 0);
    ASSERT_EQ(byte_array.load<3>(), 0);
    ASSERT_TRUE(byte_array.compare_exchange<2>(0, 3));
    ASSERT_EQ(byte_array.load<0>(), 1);
    ASSERT_EQ(byte_array.load<1>(), 0);
    ASSERT_EQ(byte_array.load<2>(), 3);
    ASSERT_EQ(byte_array.load<3>(), 0);
    ASSERT_TRUE(byte_array.compare_exchange<2>(3, 2));
    ASSERT_EQ(byte_array.load<0>(), 1);
    ASSERT_EQ(byte_array.load<1>(), 0);
    ASSERT_EQ(byte_array.load<2>(), 2);
    ASSERT_EQ(byte_array.load<3>(), 0);
}

TEST(AtomicSharedEnumTest, Exchange) {
    AtomicSharedEnum<2, 1, 2, 1> byte_array;
    ASSERT_EQ(byte_array.load<0>(), 0);
    ASSERT_EQ(byte_array.load<1>(), 0);
    ASSERT_EQ(byte_array.load<2>(), 0);
    ASSERT_EQ(byte_array.load<3>(), 0);
    ASSERT_EQ(byte_array.exchange<0>(3), 0);
    ASSERT_EQ(byte_array.load<0>(), 3);
    ASSERT_EQ(byte_array.load<1>(), 0);
    ASSERT_EQ(byte_array.load<2>(), 0);
    ASSERT_EQ(byte_array.load<3>(), 0);
    ASSERT_EQ(byte_array.exchange<2>(1), 0);
    ASSERT_EQ(byte_array.load<0>(), 3);
    ASSERT_EQ(byte_array.load<1>(), 0);
    ASSERT_EQ(byte_array.load<2>(), 1);
    ASSERT_EQ(byte_array.load<3>(), 0);
    ASSERT_EQ(byte_array.exchange<3>(1), 0);
    ASSERT_EQ(byte_array.load<0>(), 3);
    ASSERT_EQ(byte_array.load<1>(), 0);
    ASSERT_EQ(byte_array.load<2>(), 1);
    ASSERT_EQ(byte_array.load<3>(), 1);
    ASSERT_EQ(byte_array.exchange<2>(2), 1);
    ASSERT_EQ(byte_array.load<0>(), 3);
    ASSERT_EQ(byte_array.load<1>(), 0);
    ASSERT_EQ(byte_array.load<2>(), 2);
    ASSERT_EQ(byte_array.load<3>(), 1);
    ASSERT_EQ(byte_array.exchange<3>(1), 1);
    ASSERT_EQ(byte_array.load<0>(), 3);
    ASSERT_EQ(byte_array.load<1>(), 0);
    ASSERT_EQ(byte_array.load<2>(), 2);
    ASSERT_EQ(byte_array.load<3>(), 1);
    ASSERT_EQ(byte_array.exchange<1>(1), 0);
    ASSERT_EQ(byte_array.load<0>(), 3);
    ASSERT_EQ(byte_array.load<1>(), 1);
    ASSERT_EQ(byte_array.load<2>(), 2);
    ASSERT_EQ(byte_array.load<3>(), 1);
    ASSERT_EQ(byte_array.exchange<2>(0), 2);
    ASSERT_EQ(byte_array.load<0>(), 3);
    ASSERT_EQ(byte_array.load<1>(), 1);
    ASSERT_EQ(byte_array.load<2>(), 0);
    ASSERT_EQ(byte_array.load<3>(), 1);
    ASSERT_EQ(byte_array.exchange<1>(0), 1);
    ASSERT_EQ(byte_array.load<0>(), 3);
    ASSERT_EQ(byte_array.load<1>(), 0);
    ASSERT_EQ(byte_array.load<2>(), 0);
    ASSERT_EQ(byte_array.load<3>(), 1);
    ASSERT_EQ(byte_array.exchange<3>(0), 1);
    ASSERT_EQ(byte_array.load<0>(), 3);
    ASSERT_EQ(byte_array.load<1>(), 0);
    ASSERT_EQ(byte_array.load<2>(), 0);
    ASSERT_EQ(byte_array.load<3>(), 0);
    ASSERT_EQ(byte_array.exchange<2>(3), 0);
    ASSERT_EQ(byte_array.load<0>(), 3);
    ASSERT_EQ(byte_array.load<1>(), 0);
    ASSERT_EQ(byte_array.load<2>(), 3);
    ASSERT_EQ(byte_array.load<3>(), 0);
    ASSERT_EQ(byte_array.exchange<2>(2), 3);
    ASSERT_EQ(byte_array.load<0>(), 3);
    ASSERT_EQ(byte_array.load<1>(), 0);
    ASSERT_EQ(byte_array.load<2>(), 2);
    ASSERT_EQ(byte_array.load<3>(), 0);
}

TEST(AtomicSharedEnumTest, Store) {
    AtomicSharedEnum<1, 2, 1> byte_array;
    ASSERT_EQ(byte_array.load<0>(), 0);
    ASSERT_EQ(byte_array.load<1>(), 0);
    ASSERT_EQ(byte_array.load<2>(), 0);
    byte_array.store<0>(1);
    ASSERT_EQ(byte_array.load<0>(), 1);
    ASSERT_EQ(byte_array.load<1>(), 0);
    ASSERT_EQ(byte_array.load<2>(), 0);
    byte_array.store<1>(3);
    ASSERT_EQ(byte_array.load<0>(), 1);
    ASSERT_EQ(byte_array.load<1>(), 3);
    ASSERT_EQ(byte_array.load<2>(), 0);
    byte_array.store<1>(2);
    ASSERT_EQ(byte_array.load<0>(), 1);
    ASSERT_EQ(byte_array.load<1>(), 2);
    ASSERT_EQ(byte_array.load<2>(), 0);
    byte_array.store<2>(1);
    ASSERT_EQ(byte_array.load<0>(), 1);
    ASSERT_EQ(byte_array.load<1>(), 2);
    ASSERT_EQ(byte_array.load<2>(), 1);
    byte_array.store<1>(1);
    ASSERT_EQ(byte_array.load<0>(), 1);
    ASSERT_EQ(byte_array.load<1>(), 1);
    ASSERT_EQ(byte_array.load<2>(), 1);
}

TEST(AtomicSharedEnumTest, CompareExchangeIndirect) {
    AtomicSharedEnum<2, 1, 2, 1> byte_array;
    ASSERT_EQ(byte_array.load<0>(), 0);
    ASSERT_EQ(byte_array.load<1>(), 0);
    ASSERT_EQ(byte_array.load<2>(), 0);
    ASSERT_EQ(byte_array.load<3>(), 0);
    ASSERT_TRUE((byte_array.compare_exchange<1, 0>(0, 3)));
    ASSERT_EQ(byte_array.load<0>(), 3);
    ASSERT_EQ(byte_array.load<1>(), 0);
    ASSERT_EQ(byte_array.load<2>(), 0);
    ASSERT_EQ(byte_array.load<3>(), 0);
    ASSERT_TRUE((byte_array.compare_exchange<1, 0>(0, 2)));
    ASSERT_EQ(byte_array.load<0>(), 2);
    ASSERT_EQ(byte_array.load<1>(), 0);
    ASSERT_EQ(byte_array.load<2>(), 0);
    ASSERT_EQ(byte_array.load<3>(), 0);
    ASSERT_TRUE((byte_array.compare_exchange<1, 2>(0, 1)));
    ASSERT_EQ(byte_array.load<0>(), 2);
    ASSERT_EQ(byte_array.load<1>(), 0);
    ASSERT_EQ(byte_array.load<2>(), 1);
    ASSERT_EQ(byte_array.load<3>(), 0);
    ASSERT_TRUE((byte_array.compare_exchange<1, 3>(0, 1)));
    ASSERT_EQ(byte_array.load<0>(), 2);
    ASSERT_EQ(byte_array.load<1>(), 0);
    ASSERT_EQ(byte_array.load<2>(), 1);
    ASSERT_EQ(byte_array.load<3>(), 1);
    ASSERT_FALSE((byte_array.compare_exchange<2, 3>(0, 2)));
    ASSERT_EQ(byte_array.load<0>(), 2);
    ASSERT_EQ(byte_array.load<1>(), 0);
    ASSERT_EQ(byte_array.load<2>(), 1);
    ASSERT_EQ(byte_array.load<3>(), 1);
    ASSERT_TRUE((byte_array.compare_exchange<2, 1>(1, 1)));
    ASSERT_EQ(byte_array.load<0>(), 2);
    ASSERT_EQ(byte_array.load<1>(), 1);
    ASSERT_EQ(byte_array.load<2>(), 1);
    ASSERT_EQ(byte_array.load<3>(), 1);
    ASSERT_TRUE((byte_array.compare_exchange<0, 0>(2, 3)));
    ASSERT_EQ(byte_array.load<0>(), 3);
    ASSERT_EQ(byte_array.load<1>(), 1);
    ASSERT_EQ(byte_array.load<2>(), 1);
    ASSERT_EQ(byte_array.load<3>(), 1);
    ASSERT_TRUE((byte_array.compare_exchange<0, 2>(3, 3)));
    ASSERT_EQ(byte_array.load<0>(), 3);
    ASSERT_EQ(byte_array.load<1>(), 1);
    ASSERT_EQ(byte_array.load<2>(), 3);
    ASSERT_EQ(byte_array.load<3>(), 1);
    ASSERT_TRUE((byte_array.compare_exchange<0, 1>(3, 0)));
    ASSERT_EQ(byte_array.load<0>(), 3);
    ASSERT_EQ(byte_array.load<1>(), 0);
    ASSERT_EQ(byte_array.load<2>(), 3);
    ASSERT_EQ(byte_array.load<3>(), 1);
    ASSERT_TRUE((byte_array.compare_exchange<3, 3>(1, 0)));
    ASSERT_EQ(byte_array.load<0>(), 3);
    ASSERT_EQ(byte_array.load<1>(), 0);
    ASSERT_EQ(byte_array.load<2>(), 3);
    ASSERT_EQ(byte_array.load<3>(), 0);
    ASSERT_TRUE((byte_array.compare_exchange<0, 2>(3, 0)));
    ASSERT_EQ(byte_array.load<0>(), 3);
    ASSERT_EQ(byte_array.load<1>(), 0);
    ASSERT_EQ(byte_array.load<2>(), 0);
    ASSERT_EQ(byte_array.load<3>(), 0);
    ASSERT_FALSE((byte_array.compare_exchange<1, 2>(1, 0)));
    ASSERT_EQ(byte_array.load<0>(), 3);
    ASSERT_EQ(byte_array.load<1>(), 0);
    ASSERT_EQ(byte_array.load<2>(), 0);
    ASSERT_EQ(byte_array.load<3>(), 0);
}

TEST(AtomicSharedEnumTest, TwoByteEnum) {
    AtomicSharedEnum<7, 9> byte_array;
    ASSERT_EQ(byte_array.load<0>(), 0);
    ASSERT_EQ(byte_array.load<1>(), 0);
    byte_array.store<0>((1 << 7) - 1);
    ASSERT_EQ(byte_array.load<0>(), (1 << 7) - 1);
    ASSERT_EQ(byte_array.load<1>(), 0);
    ASSERT_TRUE(byte_array.compare_exchange<1>(0, (1 << 9) - 1));
    ASSERT_EQ(byte_array.load<0>(), (1 << 7) - 1);
    ASSERT_EQ(byte_array.load<1>(), (1 << 9) - 1);
    ASSERT_EQ(byte_array.exchange<0>(0), (1 << 7) - 1);
    ASSERT_EQ(byte_array.load<0>(), 0);
    ASSERT_EQ(byte_array.load<1>(), (1 << 9) - 1);
    ASSERT_FALSE((byte_array.compare_exchange<1, 0>((1 << 8) - 1, 0)));
    ASSERT_EQ(byte_array.load<0>(), 0);
    ASSERT_EQ(byte_array.load<1>(), (1 << 9) - 1);
    byte_array.store<1>(0);
    ASSERT_EQ(byte_array.load<0>(), 0);
    ASSERT_EQ(byte_array.load<1>(), 0);
}

TEST(AtomicSharedEnumTest, SizeTest) {
    static_assert(
        std::is_same_v<AtomicSharedEnum<1, 2, 1, 2>::T, std::uint8_t>);
    static_assert(
        std::is_same_v<AtomicSharedEnum<1, 2, 1, 2, 2>::T, std::uint8_t>);
    static_assert(
        std::is_same_v<AtomicSharedEnum<1, 2, 1, 2, 1, 2>::T, std::uint16_t>);
    static_assert(
        std::is_same_v<AtomicSharedEnum<1, 2, 1, 2, 1, 2>::T, std::uint16_t>);
    static_assert(std::is_same_v<AtomicSharedEnum<8, 8>::T, std::uint16_t>);
    static_assert(std::is_same_v<AtomicSharedEnum<8, 1, 8>::T, std::uint32_t>);
    static_assert(
        std::is_same_v<AtomicSharedEnum<8, 8, 8, 8>::T, std::uint32_t>);
}
