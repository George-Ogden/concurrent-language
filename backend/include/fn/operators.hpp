#pragma once

#include "data_structures/lazy.tpp"
#include "fn/fn.tpp"
#include "system/work_manager.hpp"
#include "types/builtin.hpp"
#include "types/utils.hpp"

#include <compare>

#define Binary_Int_Int_Int_Op__BuiltIn(fn)                                     \
    struct fn##_Fn : public EasyCloneFn<fn##_Fn, Int, Int, Int> {              \
        using EasyCloneFn<fn##_Fn, Int, Int, Int>::EasyCloneFn;                \
        LazyT<Int> body(LazyT<Int> &x, LazyT<Int> &y) override {               \
            return fn(x, y);                                                   \
        }                                                                      \
    };

#define Unary_Int_Int_Op__BuiltIn(fn)                                          \
    struct fn##_Fn : public EasyCloneFn<fn##_Fn, Int, Int> {                   \
        using EasyCloneFn<fn##_Fn, Int, Int>::EasyCloneFn;                     \
        LazyT<Int> body(LazyT<Int> &x) override { return fn(x); }              \
    };

#define Unary_Bool_Bool_Op__BuiltIn(fn)                                        \
    struct fn##_Fn : public EasyCloneFn<fn##_Fn, Bool, Bool> {                 \
        using EasyCloneFn<fn##_Fn, Bool, Bool>::EasyCloneFn;                   \
        LazyT<Bool> body(LazyT<Bool> &x) override { return fn(x); }            \
    };

#define Binary_Int_Int_Bool_Op__BuiltIn(fn)                                    \
    struct fn##_Fn : public EasyCloneFn<fn##_Fn, Bool, Int, Int> {             \
        using EasyCloneFn<fn##_Fn, Bool, Int, Int>::EasyCloneFn;               \
        LazyT<Bool> body(LazyT<Int> &x, LazyT<Int> &y) override {              \
            return fn(x, y);                                                   \
        }                                                                      \
    };

LazyT<Int> Plus__BuiltIn(LazyT<Int> x, LazyT<Int> y) {
    WorkManager::await(x, y);
    return std::make_shared<LazyConstant<Int>>(x->value() + y->value());
}

LazyT<Int> Minus__BuiltIn(LazyT<Int> x, LazyT<Int> y) {
    WorkManager::await(x, y);
    return std::make_shared<LazyConstant<Int>>(x->value() - y->value());
}

LazyT<Int> Multiply__BuiltIn(LazyT<Int> x, LazyT<Int> y) {
    WorkManager::await(x, y);
    return std::make_shared<LazyConstant<Int>>(x->value() * y->value());
}

LazyT<Int> Divide__BuiltIn(LazyT<Int> x, LazyT<Int> y) {
    WorkManager::await(x, y);
    return std::make_shared<LazyConstant<Int>>(x->value() / y->value());
}

LazyT<Int> Exponentiate__BuiltIn(LazyT<Int> x, LazyT<Int> y) {
    WorkManager::await(y);
    if (y->value() < 0)
        return std::make_shared<LazyConstant<Int>>(0);
    WorkManager::await(x);
    Int res = 1, base = x->value(), exp = y->value();
    while (exp) {
        if (exp & 1)
            res *= base;
        exp >>= 1;
        base *= base;
    }
    return std::make_shared<LazyConstant<Int>>(res);
}

LazyT<Int> Modulo__BuiltIn(LazyT<Int> x, LazyT<Int> y) {
    WorkManager::await(x, y);
    return std::make_shared<LazyConstant<Int>>(x->value() % y->value());
}

LazyT<Int> Right_Shift__BuiltIn(LazyT<Int> x, LazyT<Int> y) {
    WorkManager::await(x, y);
    return std::make_shared<LazyConstant<Int>>(x->value() >> y->value());
}

LazyT<Int> Left_Shift__BuiltIn(LazyT<Int> x, LazyT<Int> y) {
    WorkManager::await(x, y);
    return std::make_shared<LazyConstant<Int>>(x->value() << y->value());
}

LazyT<Int> Spaceship__BuiltIn(LazyT<Int> x, LazyT<Int> y) {
    WorkManager::await(x, y);
    const auto o = (x->value() <=> y->value());
    if (o == std::strong_ordering::less)
        return std::make_shared<LazyConstant<Int>>(-1);
    if (o == std::strong_ordering::greater)
        return std::make_shared<LazyConstant<Int>>(1);
    return std::make_shared<LazyConstant<Int>>(0);
}

LazyT<Int> Bitwise_And__BuiltIn(LazyT<Int> x, LazyT<Int> y) {
    WorkManager::await(x, y);
    return std::make_shared<LazyConstant<Int>>(x->value() & y->value());
}

LazyT<Int> Bitwise_Or__BuiltIn(LazyT<Int> x, LazyT<Int> y) {
    WorkManager::await(x, y);
    return std::make_shared<LazyConstant<Int>>(x->value() | y->value());
}

LazyT<Int> Bitwise_Xor__BuiltIn(LazyT<Int> x, LazyT<Int> y) {
    WorkManager::await(x, y);
    return std::make_shared<LazyConstant<Int>>(x->value() ^ y->value());
}

LazyT<Int> Increment__BuiltIn(LazyT<Int> x) {
    WorkManager::await(x);
    return std::make_shared<LazyConstant<Int>>(x->value() + 1);
}

LazyT<Int> Decrement__BuiltIn(LazyT<Int> x) {
    WorkManager::await(x);
    return std::make_shared<LazyConstant<Int>>(x->value() - 1);
}

LazyT<Bool> Negation__BuiltIn(LazyT<Bool> x) {
    WorkManager::await(x);
    return std::make_shared<LazyConstant<Bool>>(!x->value());
}

LazyT<Bool> Comparison_LT__BuiltIn(LazyT<Int> x, LazyT<Int> y) {
    WorkManager::await(x, y);
    return std::make_shared<LazyConstant<Bool>>(x->value() < y->value());
}

LazyT<Bool> Comparison_LE__BuiltIn(LazyT<Int> x, LazyT<Int> y) {
    WorkManager::await(x, y);
    return std::make_shared<LazyConstant<Bool>>(x->value() <= y->value());
}

LazyT<Bool> Comparison_EQ__BuiltIn(LazyT<Int> x, LazyT<Int> y) {
    WorkManager::await(x, y);
    return std::make_shared<LazyConstant<Bool>>(x->value() == y->value());
}

LazyT<Bool> Comparison_NE__BuiltIn(LazyT<Int> x, LazyT<Int> y) {
    WorkManager::await(x, y);
    return std::make_shared<LazyConstant<Bool>>(x->value() != y->value());
}

LazyT<Bool> Comparison_GT__BuiltIn(LazyT<Int> x, LazyT<Int> y) {
    WorkManager::await(x, y);
    return std::make_shared<LazyConstant<Bool>>(x->value() > y->value());
}

LazyT<Bool> Comparison_GE__BuiltIn(LazyT<Int> x, LazyT<Int> y) {
    WorkManager::await(x, y);
    return std::make_shared<LazyConstant<Bool>>(x->value() >= y->value());
}

Binary_Int_Int_Int_Op__BuiltIn(Plus__BuiltIn);
Binary_Int_Int_Int_Op__BuiltIn(Minus__BuiltIn);
Binary_Int_Int_Int_Op__BuiltIn(Multiply__BuiltIn);
Binary_Int_Int_Int_Op__BuiltIn(Divide__BuiltIn);
Binary_Int_Int_Int_Op__BuiltIn(Exponentiate__BuiltIn);
Binary_Int_Int_Int_Op__BuiltIn(Modulo__BuiltIn);
Binary_Int_Int_Int_Op__BuiltIn(Right_Shift__BuiltIn);
Binary_Int_Int_Int_Op__BuiltIn(Left_Shift__BuiltIn);
Binary_Int_Int_Int_Op__BuiltIn(Spaceship__BuiltIn);
Binary_Int_Int_Int_Op__BuiltIn(Bitwise_And__BuiltIn);
Binary_Int_Int_Int_Op__BuiltIn(Bitwise_Or__BuiltIn);
Binary_Int_Int_Int_Op__BuiltIn(Bitwise_Xor__BuiltIn);

Unary_Int_Int_Op__BuiltIn(Increment__BuiltIn);
Unary_Int_Int_Op__BuiltIn(Decrement__BuiltIn);

Unary_Bool_Bool_Op__BuiltIn(Negation__BuiltIn);

Binary_Int_Int_Bool_Op__BuiltIn(Comparison_LT__BuiltIn);
Binary_Int_Int_Bool_Op__BuiltIn(Comparison_LE__BuiltIn);
Binary_Int_Int_Bool_Op__BuiltIn(Comparison_EQ__BuiltIn);
Binary_Int_Int_Bool_Op__BuiltIn(Comparison_NE__BuiltIn);
Binary_Int_Int_Bool_Op__BuiltIn(Comparison_GT__BuiltIn);
Binary_Int_Int_Bool_Op__BuiltIn(Comparison_GE__BuiltIn);
