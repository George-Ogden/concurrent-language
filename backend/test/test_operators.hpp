#pragma once

#include "fn/fn.hpp"
#include "fn/operators.hpp"
#include "system/work_manager.hpp"

#include <gtest/gtest.h>

#include <compare>
#include <functional>

class BinaryOperatorsTests
    : public ::testing::TestWithParam<std::tuple<
          ParametricFn<Int, Int, Int> *, std::function<Int(Int, Int)>>> {};

TEST_P(BinaryOperatorsTests, OperatorCorrectness) {
    auto &[fn, op] = GetParam();

    for (Int x : std::vector<Int>{-1000000009LL, -55, 24, 200, 10024,
                                  1000000000224LL}) {
        for (Int y : {-8, 4, 3, 17}) {
            fn->args = Fn::reference_all(x, y);

            WorkManager::run(fn);
            Int expected = op(x, y);
            ASSERT_EQ(fn->ret, expected);
        }
    }
}

INSTANTIATE_TEST_SUITE_P(
    BinaryOperators, BinaryOperatorsTests,
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

class UnaryOperatorsTests
    : public ::testing::TestWithParam<
          std::tuple<ParametricFn<Int, Int> *, std::function<Int(Int)>>> {};

TEST_P(UnaryOperatorsTests, OperatorCorrectness) {
    auto &[fn, op] = GetParam();

    for (Int x : std::vector<Int>{-1000000009LL, -55, 24, 200, 10024,
                                  1000000000224LL}) {
        fn->args = Fn::reference_all(x);

        WorkManager::run(fn);
        Int expected = op(x);
        ASSERT_EQ(fn->ret, expected);
    }
}
INSTANTIATE_TEST_SUITE_P(
    UnaryOperators, UnaryOperatorsTests,
    ::testing::Values(
        std::make_tuple(new Increment__BuiltIn{}, [](Int x) { return ++x; }),
        std::make_tuple(new Decrement__BuiltIn{}, [](Int x) { return --x; })));

class BinaryComparisonsTests
    : public ::testing::TestWithParam<std::tuple<
          ParametricFn<Bool, Int, Int> *, std::function<Bool(Int, Int)>>> {};

TEST_P(BinaryComparisonsTests, OperatorCorrectness) {
    auto &[fn, op] = GetParam();

    const std::vector<Int> xs{-1000000009LL, -55,   24,
                              200,           10024, 1000000000224LL};
    for (Int x : xs) {
        for (Int y : xs) {
            fn->args = Fn::reference_all(x, y);

            WorkManager::run(fn);
            Bool expected = op(x, y);
            ASSERT_EQ(fn->ret, expected);
        }
    }
}

INSTANTIATE_TEST_SUITE_P(
    BinaryComparisons, BinaryComparisonsTests,
    ::testing::Values(
        std::make_tuple(new Comparison_LT__BuiltIn{}, std::less<Int>()),
        std::make_tuple(new Comparison_GT__BuiltIn{}, std::greater<Int>()),
        std::make_tuple(new Comparison_LE__BuiltIn{}, std::less_equal<Int>()),
        std::make_tuple(new Comparison_GE__BuiltIn{},
                        std::greater_equal<Int>()),
        std::make_tuple(new Comparison_EQ__BuiltIn{}, std::equal_to<Int>()),
        std::make_tuple(new Comparison_NE__BuiltIn{},
                        std::not_equal_to<Int>())));
