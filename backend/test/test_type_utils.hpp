#pragma once

#include "types/compound.hpp"
#include "types/utils.hpp"

#include <gtest/gtest.h>

#include <chrono>

TEST(TupleAllocationTests, IntegerAllocationTest) {
    TupleT<Int, Int> basic = std::make_tuple(8, 4);
    TupleT<Int, Int *> non_basic = create_references<TupleT<Int, Int *>>(basic);
    ASSERT_EQ(std::get<0>(non_basic), 8);
    ASSERT_EQ(*std::get<1>(non_basic), 4);
}

TEST(TupleAllocationTests, EmptySequence) {
    TupleT<> basic = std::make_tuple();
    TupleT<> non_basic = create_references<TupleT<>>(basic);
    ASSERT_EQ(basic, non_basic);
}

TEST(TupleAllocationTests, NonTupleSequence) {
    Int basic = 4;
    Int non_basic = create_references<Int>(basic);
    ASSERT_EQ(basic, non_basic);
}

TEST(TupleDeAllocationTests, IntegerDeAllocationTest) {
    TupleT<Int, Int *> non_basic = std::make_tuple(8, new Int{4});
    TupleT<Int, Int> basic = destroy_references(non_basic);
    ASSERT_EQ(std::get<0>(basic), 8);
    ASSERT_EQ(std::get<1>(basic), 4);
}

TEST(TupleDeAllocationTests, EmptySequence) {
    TupleT<> non_basic = std::make_tuple();
    TupleT<> basic = destroy_references<TupleT<>>(non_basic);
    ASSERT_EQ(basic, non_basic);
}

TEST(TupleDeAllocationTests, NonTupleSequence) {
    Int non_basic = 4;
    Int basic = destroy_references<Int>(non_basic);
    ASSERT_EQ(basic, non_basic);
}
