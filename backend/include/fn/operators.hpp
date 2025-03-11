#pragma once

#include "fn/fn_gen.tpp"
#include "fn/fn_inst.tpp"
#include "lazy/types.hpp"
#include "system/work_manager.tpp"
#include "types/builtin.hpp"
#include "types/utils.hpp"

#include <compare>
#include <memory>

#define Binary_Int_Int_Int_Op__BuiltIn(fn, size)                               \
    class fn##_I : public TypedFnI<Int, Int, Int> {                            \
      protected:                                                               \
        LazyT<Int> body(LazyT<Int> &x, LazyT<Int> &y) override {               \
            return fn(x, y);                                                   \
        }                                                                      \
                                                                               \
      public:                                                                  \
        using TypedFnI<Int, Int, Int>::TypedFnI;                               \
        static std::unique_ptr<TypedFnI<Int, Int, Int>>                        \
        init(const ArgsT &args) {                                              \
            return std::make_unique<fn##_I>(args);                             \
        }                                                                      \
        constexpr std::size_t lower_size_bound() const override {              \
            return size;                                                       \
        };                                                                     \
        constexpr std::size_t upper_size_bound() const override {              \
            return size;                                                       \
        };                                                                     \
    };                                                                         \
    FnT<Int, Int, Int> fn##_G =                                                \
        std::make_shared<TypedClosureG<Empty, Int, Int, Int>>(fn##_I::init);

#define Unary_Int_Int_Op__BuiltIn(fn, size)                                    \
    class fn##_I : public TypedFnI<Int, Int> {                                 \
      protected:                                                               \
        LazyT<Int> body(LazyT<Int> &x) override { return fn(x); }              \
                                                                               \
      public:                                                                  \
        using TypedFnI<Int, Int>::TypedFnI;                                    \
        static std::unique_ptr<TypedFnI<Int, Int>> init(const ArgsT &args) {   \
            return std::make_unique<fn##_I>(args);                             \
        }                                                                      \
        constexpr std::size_t lower_size_bound() const override {              \
            return size;                                                       \
        };                                                                     \
        constexpr std::size_t upper_size_bound() const override {              \
            return size;                                                       \
        };                                                                     \
    };                                                                         \
    FnT<Int, Int> fn##_G =                                                     \
        std::make_shared<TypedClosureG<Empty, Int, Int>>(fn##_I::init);

#define Unary_Bool_Bool_Op__BuiltIn(fn, size)                                  \
    class fn##_I : public TypedFnI<Bool, Bool> {                               \
      protected:                                                               \
        LazyT<Bool> body(LazyT<Bool> &x) override { return fn(x); }            \
                                                                               \
      public:                                                                  \
        using TypedFnI<Bool, Bool>::TypedFnI;                                  \
        static std::unique_ptr<TypedFnI<Bool, Bool>> init(const ArgsT &args) { \
            return std::make_unique<fn##_I>(args);                             \
        }                                                                      \
        constexpr std::size_t lower_size_bound() const override {              \
            return size;                                                       \
        };                                                                     \
        constexpr std::size_t upper_size_bound() const override {              \
            return size;                                                       \
        };                                                                     \
    };                                                                         \
    FnT<Bool, Bool> fn##_G =                                                   \
        std::make_shared<TypedClosureG<Empty, Bool, Bool>>(fn##_I::init);

#define Binary_Int_Int_Bool_Op__BuiltIn(fn, size)                              \
    class fn##_I : public TypedFnI<Bool, Int, Int> {                           \
      protected:                                                               \
        LazyT<Bool> body(LazyT<Int> &x, LazyT<Int> &y) override {              \
            return fn(x, y);                                                   \
        }                                                                      \
                                                                               \
      public:                                                                  \
        using TypedFnI<Bool, Int, Int>::TypedFnI;                              \
        static std::unique_ptr<TypedFnI<Bool, Int, Int>>                       \
        init(const ArgsT &args) {                                              \
            return std::make_unique<fn##_I>(args);                             \
        }                                                                      \
        constexpr std::size_t lower_size_bound() const override {              \
            return size;                                                       \
        };                                                                     \
        constexpr std::size_t upper_size_bound() const override {              \
            return size;                                                       \
        };                                                                     \
    };                                                                         \
    FnT<Bool, Int, Int> fn##_G =                                               \
        std::make_shared<TypedClosureG<Empty, Bool, Int, Int>>(fn##_I::init);

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

Binary_Int_Int_Int_Op__BuiltIn(Plus__BuiltIn, 9);
Binary_Int_Int_Int_Op__BuiltIn(Minus__BuiltIn, 9);
Binary_Int_Int_Int_Op__BuiltIn(Multiply__BuiltIn, 9);
Binary_Int_Int_Int_Op__BuiltIn(Divide__BuiltIn, 10);
Binary_Int_Int_Int_Op__BuiltIn(Exponentiate__BuiltIn, 12);
Binary_Int_Int_Int_Op__BuiltIn(Modulo__BuiltIn, 10);
Binary_Int_Int_Int_Op__BuiltIn(Right_Shift__BuiltIn, 9);
Binary_Int_Int_Int_Op__BuiltIn(Left_Shift__BuiltIn, 9);
Binary_Int_Int_Int_Op__BuiltIn(Spaceship__BuiltIn, 9);
Binary_Int_Int_Int_Op__BuiltIn(Bitwise_And__BuiltIn, 9);
Binary_Int_Int_Int_Op__BuiltIn(Bitwise_Or__BuiltIn, 9);
Binary_Int_Int_Int_Op__BuiltIn(Bitwise_Xor__BuiltIn, 9);

Unary_Int_Int_Op__BuiltIn(Increment__BuiltIn, 8);
Unary_Int_Int_Op__BuiltIn(Decrement__BuiltIn, 8);

Unary_Bool_Bool_Op__BuiltIn(Negation__BuiltIn, 8);

Binary_Int_Int_Bool_Op__BuiltIn(Comparison_LT__BuiltIn, 9);
Binary_Int_Int_Bool_Op__BuiltIn(Comparison_LE__BuiltIn, 9);
Binary_Int_Int_Bool_Op__BuiltIn(Comparison_EQ__BuiltIn, 9);
Binary_Int_Int_Bool_Op__BuiltIn(Comparison_NE__BuiltIn, 9);
Binary_Int_Int_Bool_Op__BuiltIn(Comparison_GT__BuiltIn, 9);
Binary_Int_Int_Bool_Op__BuiltIn(Comparison_GE__BuiltIn, 9);
