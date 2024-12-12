#pragma once

#include "fn.hpp"

#include <gtest/gtest.h>

struct IdentityInt : ParametricFn<int, int> {
    void body() override { *ret = *std::get<0>(args); }
};

GTEST_TEST(FnTests, IdentityTest) {
    IdentityInt id{};
    int x = 5, r = 0;
    std::get<0>(id.args) = &x;
    id.ret = &r;
    id.run();
    ASSERT_EQ(r, 5);
    ASSERT_EQ(x, 5);
}
