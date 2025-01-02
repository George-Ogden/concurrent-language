#pragma once

#include "fn/fn.hpp"
#include "types/predefined.hpp"

#include <compare>
#include <type_traits>
#include <utility>

template <typename R, typename... Ts>
struct Op__BuiltIn : public ParametricFn<R, Ts...> {
    using ParametricFn<R, Ts...>::ParametricFn;
    std::decay_t<R> body(
        std::add_const_t<std::add_lvalue_reference_t<std::decay_t<Ts>>>... args)
        override {
        return op(args...);
    };
    virtual std::decay_t<R> op(std::add_const_t<Ts>...) const = 0;
    template <typename Tuple> auto expand_values(Tuple &&t) const {
        return []<std::size_t... I>(auto &&t, std::index_sequence<I...>) {
            return std::make_tuple(
                (std::get<I>(std::forward<decltype(t)>(t)).value())...);
        }
        (std::forward<Tuple>(t),
         std::make_index_sequence<
             std::tuple_size_v<std::remove_reference_t<Tuple>>>{});
    }
};

using Unary_Int_Int_Op__BuiltIn = Op__BuiltIn<Int, Int>;
using Binary_Int_Int_Int_Op__BuiltIn = Op__BuiltIn<Int, Int, Int>;
using Binary_Int_Int_Bool_Op__BuiltIn = Op__BuiltIn<Bool, Int, Int>;

struct Plus__BuiltIn : public Binary_Int_Int_Int_Op__BuiltIn {
    Int op(const Int x, const Int y) const override { return x + y; }
};

struct Minus__BuiltIn : public Binary_Int_Int_Int_Op__BuiltIn {
    Int op(const Int x, const Int y) const override { return x - y; }
};

struct Multiply__BuiltIn : public Binary_Int_Int_Int_Op__BuiltIn {
    Int op(const Int x, const Int y) const override { return x * y; }
};

struct Divide__BuiltIn : public Binary_Int_Int_Int_Op__BuiltIn {
    Int op(const Int x, const Int y) const override { return x / y; }
};

struct Exponentiate__BuiltIn : public Binary_Int_Int_Int_Op__BuiltIn {
    Int op(const Int x, const Int y) const override {
        if (y < 0)
            return 0;
        Int res = 1, base = x, exp = y;
        while (exp) {
            if (exp & 1)
                res *= base;
            exp >>= 1;
            base *= base;
        }
        return res;
    }
};

struct Modulo__BuiltIn : public Binary_Int_Int_Int_Op__BuiltIn {
    Int op(const Int x, const Int y) const override { return x % y; }
};

struct Left_Shift__BuiltIn : public Binary_Int_Int_Int_Op__BuiltIn {
    Int op(const Int x, const Int y) const override { return x << y; }
};

struct Right_Shift__BuiltIn : public Binary_Int_Int_Int_Op__BuiltIn {
    Int op(const Int x, const Int y) const override { return x >> y; }
};

struct Spaceship__BuiltIn : public Binary_Int_Int_Int_Op__BuiltIn {
    Int op(const Int x, const Int y) const override {
        const auto o = (x <=> y);
        if (o == std::strong_ordering::less)
            return -1;
        if (o == std::strong_ordering::greater)
            return 1;
        return 0;
    }
};

struct Bitwise_And__BuiltIn : public Binary_Int_Int_Int_Op__BuiltIn {
    Int op(const Int x, const Int y) const override { return x & y; }
};

struct Bitwise_Or__BuiltIn : public Binary_Int_Int_Int_Op__BuiltIn {
    Int op(const Int x, const Int y) const override { return x | y; }
};

struct Bitwise_Xor__BuiltIn : public Binary_Int_Int_Int_Op__BuiltIn {
    Int op(const Int x, const Int y) const override { return x ^ y; }
};

struct Increment__BuiltIn : public Unary_Int_Int_Op__BuiltIn {
    Int op(const Int x) const override { return x + 1; }
};

struct Decrement__BuiltIn : public Unary_Int_Int_Op__BuiltIn {
    Int op(const Int x) const override { return x - 1; }
};

struct Comparison_LT__BuiltIn : public Binary_Int_Int_Bool_Op__BuiltIn {
    Bool op(const Int x, const Int y) const override { return x < y; }
};

struct Comparison_LE__BuiltIn : public Binary_Int_Int_Bool_Op__BuiltIn {
    Bool op(const Int x, const Int y) const override { return x <= y; }
};

struct Comparison_GT__BuiltIn : public Binary_Int_Int_Bool_Op__BuiltIn {
    Bool op(const Int x, const Int y) const override { return x > y; }
};

struct Comparison_GE__BuiltIn : public Binary_Int_Int_Bool_Op__BuiltIn {
    Bool op(const Int x, const Int y) const override { return x >= y; }
};

struct Comparison_EQ__BuiltIn : public Binary_Int_Int_Bool_Op__BuiltIn {
    Bool op(const Int x, const Int y) const override { return x == y; }
};

struct Comparison_NE__BuiltIn : public Binary_Int_Int_Bool_Op__BuiltIn {
    Bool op(const Int x, const Int y) const override { return x != y; }
};
