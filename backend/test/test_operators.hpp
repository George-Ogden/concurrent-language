#pragma once

#include "data_structures/lazy.tpp"
#include "fn/fn.tpp"
#include "fn/operators.hpp"
#include "system/work_manager.tpp"
#include "types/compound.hpp"

#include <gtest/gtest.h>

#include <compare>
#include <functional>
#include <vector>

class BinaryOperatorsTests
    : public ::testing::TestWithParam<
          std::tuple<FnT<Int, Int, Int>, std::function<Int(Int, Int)>>> {};

TEST_P(BinaryOperatorsTests, OperatorCorrectness) {
    auto &[op, validate] = GetParam();

    for (Int x : std::vector<Int>{-1000000009LL, -55, 24, 200, 10024,
                                  1000000000224LL}) {
        for (Int y : {-8, 4, 3, 17}) {
            auto result =
                WorkManager::run(op, make_lazy<Int>(x), make_lazy<Int>(y));
            Int expected = validate(x, y);
            ASSERT_EQ(result->value(), expected);
        }
    }
}

INSTANTIATE_TEST_SUITE_P(
    BinaryOperators, BinaryOperatorsTests,
    ::testing::Values(
        std::make_tuple(Plus__BuiltIn_Fn, std::plus<Int>()),
        std::make_tuple(Minus__BuiltIn_Fn, std::minus<Int>()),
        std::make_tuple(Multiply__BuiltIn_Fn, std::multiplies<Int>()),
        std::make_tuple(Divide__BuiltIn_Fn, std::divides<Int>()),
        std::make_tuple(Exponentiate__BuiltIn_Fn,
                        [](Int x, Int y) {
                            if (y < 0)
                                return static_cast<Int>(0);
                            Int res = 1;
                            for (Int i = 0; i < y; i++) {
                                res *= x;
                            }
                            return res;
                        }),
        std::make_tuple(Modulo__BuiltIn_Fn, std::modulus<Int>()),
        std::make_tuple(Left_Shift__BuiltIn_Fn,
                        [](Int x, Int y) { return x << y; }),
        std::make_tuple(Right_Shift__BuiltIn_Fn,
                        [](Int x, Int y) { return x >> y; }),
        std::make_tuple(Spaceship__BuiltIn_Fn,
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
        std::make_tuple(Bitwise_And__BuiltIn_Fn, std::bit_and<Int>()),
        std::make_tuple(Bitwise_Or__BuiltIn_Fn, std::bit_or<Int>()),
        std::make_tuple(Bitwise_Xor__BuiltIn_Fn, std::bit_xor<Int>())));

class UnaryOperatorsTests
    : public ::testing::TestWithParam<
          std::tuple<FnT<Int, Int>, std::function<Int(Int)>>> {};

TEST_P(UnaryOperatorsTests, OperatorCorrectness) {
    auto &[op, validate] = GetParam();

    for (Int x : std::vector<Int>{-1000000009LL, -55, 24, 200, 10024,
                                  1000000000224LL}) {
        auto result = WorkManager::run(op, make_lazy<Int>(x));
        Int expected = validate(x);
        ASSERT_EQ(result->value(), expected);
    }
}

INSTANTIATE_TEST_SUITE_P(
    UnaryOperators, UnaryOperatorsTests,
    ::testing::Values(
        std::make_tuple(Increment__BuiltIn_Fn, [](Int x) { return ++x; }),
        std::make_tuple(Decrement__BuiltIn_Fn, [](Int x) { return --x; })));

class BinaryComparisonsTests
    : public ::testing::TestWithParam<
          std::tuple<FnT<Bool, Int, Int>, std::function<Bool(Int, Int)>>> {};

TEST_P(BinaryComparisonsTests, OperatorCorrectness) {
    auto &[op, validate] = GetParam();

    const std::vector<Int> xs{-1000000009LL, -55,   24,
                              200,           10024, 1000000000224LL};
    for (Int x : xs) {
        for (Int y : xs) {
            auto result =
                WorkManager::run(op, make_lazy<Int>(x), make_lazy<Int>(y));
            Bool expected = validate(x, y);
            ASSERT_EQ(result->value(), expected);
        }
    }
}

INSTANTIATE_TEST_SUITE_P(
    BinaryComparisons, BinaryComparisonsTests,
    ::testing::Values(
        std::make_tuple(Comparison_LT__BuiltIn_Fn, std::less<Int>()),
        std::make_tuple(Comparison_GT__BuiltIn_Fn, std::greater<Int>()),
        std::make_tuple(Comparison_LE__BuiltIn_Fn, std::less_equal<Int>()),
        std::make_tuple(Comparison_GE__BuiltIn_Fn, std::greater_equal<Int>()),
        std::make_tuple(Comparison_EQ__BuiltIn_Fn, std::equal_to<Int>()),
        std::make_tuple(Comparison_NE__BuiltIn_Fn, std::not_equal_to<Int>())));

TEST(NegationTests, OperatorCorrectness) {
    auto fn = Negation__BuiltIn_Fn;
    {
        auto result = WorkManager::run(fn, make_lazy<Bool>(true));
        ASSERT_EQ(result->value(), false);
    }
    {
        auto result = WorkManager::run(fn, make_lazy<Bool>(false));
        ASSERT_EQ(result->value(), true);
    }
}
