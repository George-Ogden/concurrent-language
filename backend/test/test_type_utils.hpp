#pragma once

#include "types/builtin.hpp"
#include "types/compound.hpp"
#include "types/utils.hpp"

#include <gtest/gtest.h>

#include <chrono>
#include <memory>

TEST(TupleAllocationTests, IntegerAllocationTest) {
    TupleT<Int, Int> basic = std::make_tuple(8, 4);
    TupleT<Int, std::shared_ptr<Int>> non_basic =
        create_references<TupleT<Int, std::shared_ptr<Int>>>(basic);
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
    TupleT<Int, std::shared_ptr<Int>> non_basic =
        std::make_tuple(8, std::make_shared<Int>(4));
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

TEST(VariantDestructorTests, ContainedIntegerTest) {
    VariantT<Int, std::shared_ptr<Int>> v;
    v.tag = 0;
    *reinterpret_cast<Int *>(&v.value) = 4LL;
    ASSERT_EQ(*reinterpret_cast<Int *>(&v.value), 4LL);
}

TEST(VariantDestructorTests, ContainedSharedPtrTest) {
    VariantT<Int, std::shared_ptr<Int>> v;
    {
        std::shared_ptr<Int> p = std::make_shared<Int>(4);
        v.tag = 1;
        *reinterpret_cast<std::shared_ptr<Int> *>(&v.value) = p;
    }
    ASSERT_EQ(**reinterpret_cast<std::shared_ptr<Int> *>(&v.value), 4);
}

TEST(VariantDestructorTests, DoubleReferenceTestWithoutVariant) {
    using T = std::shared_ptr<Int>;
    using U = std::shared_ptr<T>;
    T t;
    t = std::make_shared<Int>(4LL);

    U u;
    u = std::make_shared<T>(t);
    ASSERT_EQ(*t, 4);
    ASSERT_EQ(**u, 4);
}

TEST(VariantDestructorTests, DoubleReferenceTest) {
    using T = VariantT<std::shared_ptr<Int>>;
    T t;
    t.tag = 0;
    new (&t.value) std::shared_ptr<Int>(std::make_shared<Int>(4LL));

    using U = VariantT<std::shared_ptr<T>>;
    U u;
    u.tag = 0;
    new (&u.value) std::shared_ptr<T>(std::make_shared<T>(t));
    ASSERT_EQ(**reinterpret_cast<std::shared_ptr<Int> *>(&t.value), 4);
    ASSERT_EQ(**reinterpret_cast<std::shared_ptr<Int> *>(
                  &(*reinterpret_cast<std::shared_ptr<T> *>(&u.value))->value),
              4);
}
