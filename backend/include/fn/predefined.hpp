#pragma once

#include "fn/fn.hpp"
#include "types/predefined.hpp"

#include <compare>

struct Binary_Int_Int_Int_Op__BuiltIn : public ParametricFn<Int, Int, Int> {
    void body() override { *ret = op(*std::get<0>(args), *std::get<1>(args)); }
    virtual Int op(const Int x, const Int y) const = 0;
};

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
