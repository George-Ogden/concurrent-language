#pragma once

#include "fn/fn.hpp"
#include "fn/predefined.hpp"

#include <gtest/gtest.h>

#include <compare>
#include <functional>

class FnTest
    : public ::testing::TestWithParam<std::tuple<
          ParametricFn<Int, Int, Int> *, std::function<Int(Int, Int)>>> {};

TEST_P(FnTest, Operators) {
    auto &[fn, op] = GetParam();

    for (Int x : std::vector<Int>{-1000000009LL, -55, 24, 200, 10024,
                                  1000000000224LL}) {
        for (Int y : {-8, 4, 3, 17}) {
            Int r = 0;
            fn->args = std::make_tuple(&x, &y);
            fn->ret = &r;
            ASSERT_EQ(r, 0);

            fn->run();
            Int expected = op(x, y);
            ASSERT_EQ(r, expected);
        }
    }
}

INSTANTIATE_TEST_SUITE_P(
    FnOperators, FnTest,
    ::testing::Values(
        std::make_tuple(new Plus__BuiltIn{}, std::plus<Int>()),
        std::make_tuple(new Minus__BuiltIn{}, std::minus<Int>()),
        std::make_tuple(new Multiply__BuiltIn{}, std::multiplies<Int>()),
        std::make_tuple(new Divide__BuiltIn{}, std::divides<Int>()),
        std::make_tuple(new Exponentiate__BuiltIn{},
                        [](Int x, Int y) {
                            if (y < 0)
                                return static_cast<Int>(0);
                            Int res = 1;
                            for (Int i = 0; i < y; i++) {
                                res *= x;
                            }
                            return res;
                        }),
        std::make_tuple(new Modulo__BuiltIn{}, std::modulus<Int>()),
        std::make_tuple(new Left_Shift__BuiltIn{},
                        [](Int x, Int y) { return x << y; }),
        std::make_tuple(new Right_Shift__BuiltIn{},
                        [](Int x, Int y) { return x >> y; }),
        std::make_tuple(new Spaceship__BuiltIn{},
                        [](Int x, Int y) {
                            const auto o = std::compare_three_way()(x, y);
                            if (o == std::strong_ordering::less)
                                return -1;
                            if (o == std::strong_ordering::greater)
                                return 1;
                            if (o == std::strong_ordering::equivalent)
                                return 0;
                            return 2;
                        }),
        std::make_tuple(new Bitwise_And__BuiltIn{}, std::bit_and<Int>()),
        std::make_tuple(new Bitwise_Or__BuiltIn{}, std::bit_or<Int>()),
        std::make_tuple(new Bitwise_Xor__BuiltIn{}, std::bit_xor<Int>())));
