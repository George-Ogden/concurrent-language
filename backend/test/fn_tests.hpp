#pragma once

#include "fn/fn.hpp"
#include "fn/predefined.hpp"

#include <gtest/gtest.h>

struct IdentityInt : ParametricFn<int, int> {
    void body() override { *ret = *std::get<0>(args); }
};

GTEST_TEST(FnTests, IdentityTest) {
    IdentityInt id{};
    int x = 5, r = 0;
    std::get<0>(id.args) = &x;
    id.ret = &r;
    ASSERT_EQ(r, 0);

    id.run();
    ASSERT_EQ(r, 5);
    ASSERT_EQ(x, 5);
}

GTEST_TEST(FnTests, PlusTest) {
    Plus__BuiltIn plus{};
    Int x = 5, y = 10, r = 0;
    std::get<0>(plus.args) = &x;
    std::get<1>(plus.args) = &y;
    plus.ret = &r;
    ASSERT_EQ(r, 0);

    plus.run();
    ASSERT_EQ(x, 5);
    ASSERT_EQ(y, 10);
    ASSERT_EQ(r, 15);
}
