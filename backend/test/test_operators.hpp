#pragma once

#include "fn/fn.hpp"
#include "fn/operators.hpp"
#include "system/work_manager.hpp"
#include "types/compound.hpp"

#include <gtest/gtest.h>

#include <compare>
#include <functional>

class BinaryOperatorsTests
    : public ::testing::TestWithParam<
          std::tuple<FnT<Int, Int, Int>, std::function<Int(Int, Int)>>> {};

TEST_P(BinaryOperatorsTests, OperatorCorrectness) {
    auto &[op, validate] = GetParam();

    for (Int x : std::vector<Int>{-1000000009LL, -55, 24, 200, 10024,
                                  1000000000224LL}) {
        for (Int y : {-8, 4, 3, 17}) {
            auto fn = op->clone();
            fn->args = Fn::reference_all(x, y);

            WorkManager::run(fn);
            Int expected = validate(x, y);
            ASSERT_EQ(fn->value(), expected);
        }
    }
}

INSTANTIATE_TEST_SUITE_P(
    BinaryOperators, BinaryOperatorsTests,
    ::testing::Values(
        std::make_tuple(std::make_shared<Plus__BuiltIn_Fn>(), std::plus<Int>()),
        std::make_tuple(std::make_shared<Minus__BuiltIn_Fn>(),
                        std::minus<Int>()),
        std::make_tuple(std::make_shared<Multiply__BuiltIn_Fn>(),
                        std::multiplies<Int>()),
        std::make_tuple(std::make_shared<Divide__BuiltIn_Fn>(),
                        std::divides<Int>()),
        std::make_tuple(std::make_shared<Exponentiate__BuiltIn_Fn>(),
                        [](Int x, Int y) {
                            if (y < 0)
                                return static_cast<Int>(0);
                            Int res = 1;
                            for (Int i = 0; i < y; i++) {
                                res *= x;
                            }
                            return res;
                        }),
        std::make_tuple(std::make_shared<Modulo__BuiltIn_Fn>(),
                        std::modulus<Int>()),
        std::make_tuple(std::make_shared<Left_Shift__BuiltIn_Fn>(),
                        [](Int x, Int y) { return x << y; }),
        std::make_tuple(std::make_shared<Right_Shift__BuiltIn_Fn>(),
                        [](Int x, Int y) { return x >> y; }),
        std::make_tuple(std::make_shared<Spaceship__BuiltIn_Fn>(),
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
        std::make_tuple(std::make_shared<Bitwise_And__BuiltIn_Fn>(),
                        std::bit_and<Int>()),
        std::make_tuple(std::make_shared<Bitwise_Or__BuiltIn_Fn>(),
                        std::bit_or<Int>()),
        std::make_tuple(std::make_shared<Bitwise_Xor__BuiltIn_Fn>(),
                        std::bit_xor<Int>())));

class UnaryOperatorsTests
    : public ::testing::TestWithParam<
          std::tuple<FnT<Int, Int>, std::function<Int(Int)>>> {};

TEST_P(UnaryOperatorsTests, OperatorCorrectness) {
    auto &[op, validate] = GetParam();

    for (Int x : std::vector<Int>{-1000000009LL, -55, 24, 200, 10024,
                                  1000000000224LL}) {
        auto fn = op->clone();
        fn->args = Fn::reference_all(x);

        WorkManager::run(fn);
        Int expected = validate(x);
        ASSERT_EQ(fn->value(), expected);
    }
}

INSTANTIATE_TEST_SUITE_P(
    UnaryOperators, UnaryOperatorsTests,
    ::testing::Values(std::make_tuple(std::make_shared<Increment__BuiltIn_Fn>(),
                                      [](Int x) { return ++x; }),
                      std::make_tuple(std::make_shared<Decrement__BuiltIn_Fn>(),
                                      [](Int x) { return --x; })));

class BinaryComparisonsTests
    : public ::testing::TestWithParam<
          std::tuple<FnT<Bool, Int, Int>, std::function<Bool(Int, Int)>>> {};

TEST_P(BinaryComparisonsTests, OperatorCorrectness) {
    auto &[op, validate] = GetParam();

    const std::vector<Int> xs{-1000000009LL, -55,   24,
                              200,           10024, 1000000000224LL};
    for (Int x : xs) {
        for (Int y : xs) {
            auto fn = op->clone();
            fn->args = Fn::reference_all(x, y);

            WorkManager::run(fn);
            Bool expected = validate(x, y);
            ASSERT_EQ(fn->value(), expected);
        }
    }
}

INSTANTIATE_TEST_SUITE_P(
    BinaryComparisons, BinaryComparisonsTests,
    ::testing::Values(
        std::make_tuple(std::make_shared<Comparison_LT__BuiltIn_Fn>(),
                        std::less<Int>()),
        std::make_tuple(std::make_shared<Comparison_GT__BuiltIn_Fn>(),
                        std::greater<Int>()),
        std::make_tuple(std::make_shared<Comparison_LE__BuiltIn_Fn>(),
                        std::less_equal<Int>()),
        std::make_tuple(std::make_shared<Comparison_GE__BuiltIn_Fn>(),
                        std::greater_equal<Int>()),
        std::make_tuple(std::make_shared<Comparison_EQ__BuiltIn_Fn>(),
                        std::equal_to<Int>()),
        std::make_tuple(std::make_shared<Comparison_NE__BuiltIn_Fn>(),
                        std::not_equal_to<Int>())));

TEST(NegationTests, OperatorCorrectness) {
    {
        auto fn = std::make_shared<Negation__BuiltIn_Fn>();
        auto t = true;
        fn->args = Fn::reference_all(t);

        WorkManager::run(fn);
        ASSERT_EQ(fn->value(), false);
    }
    {
        auto fn = std::make_shared<Negation__BuiltIn_Fn>();
        auto f = false;
        fn->args = Fn::reference_all(f);

        WorkManager::run(fn);
        ASSERT_EQ(fn->value(), true);
    }
}
