#pragma once

#include "types/builtin.hpp"
#include "types/compound.hpp"

#include <gtest/gtest.h>

#include <memory>
#include <type_traits>

TEST(VariantDestructorTests, ContainedIntegerTest) {
    VariantT<Int, std::shared_ptr<Int>> v{
        std::integral_constant<std::size_t, 0>(), static_cast<Int>(4LL)};
    ASSERT_EQ(*reinterpret_cast<Int *>(&v.value), 4LL);
}

TEST(VariantDestructorTests, ContainedSharedPtrTest) {
    VariantT<Int, std::shared_ptr<Int>> v{
        std::integral_constant<std::size_t, 1>(), std::make_shared<Int>(4)};
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
    T t{std::integral_constant<std::size_t, 0>(), std::make_shared<Int>(4LL)};

    using U = VariantT<std::shared_ptr<T>>;
    U u{std::integral_constant<std::size_t, 0>(), std::make_shared<T>(t)};

    ASSERT_EQ(**reinterpret_cast<std::shared_ptr<Int> *>(&t.value), 4);
    ASSERT_EQ(**reinterpret_cast<std::shared_ptr<Int> *>(
                  &(*reinterpret_cast<std::shared_ptr<T> *>(&u.value))->value),
              4);
}
