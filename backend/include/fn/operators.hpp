#pragma once

#include "data_structures/lazy.hpp"
#include "fn/fn.hpp"
#include "system/work_manager.hpp"
#include "types/builtin.hpp"

#include <compare>
#include <type_traits>
#include <utility>

template <typename R, typename... Ts>
struct Op__BuiltIn : public ParametricFn<R, Ts...> {
    using ParametricFn<R, Ts...>::ParametricFn;
    Lazy<R> *
    body(std::add_lvalue_reference_t<
         std::add_pointer_t<Lazy<std::decay_t<Ts>>>>... args) override {
        WorkManager::await(args...);
        return new LazyConstant<R>(op(args->value()...));
    };
    virtual std::decay_t<R> op(std::add_const_t<Ts>...) const = 0;
};

using Unary_Int_Int_Op__BuiltIn = Op__BuiltIn<Int, Int>;
using Unary_Bool_Bool_Op__BuiltIn = Op__BuiltIn<Bool, Bool>;
using Binary_Int_Int_Int_Op__BuiltIn = Op__BuiltIn<Int, Int, Int>;
using Binary_Int_Int_Bool_Op__BuiltIn = Op__BuiltIn<Bool, Int, Int>;

struct Plus__BuiltIn : public Binary_Int_Int_Int_Op__BuiltIn {
    using Binary_Int_Int_Int_Op__BuiltIn::Op__BuiltIn;
    Binary_Int_Int_Int_Op__BuiltIn *clone() const override {
        return new Plus__BuiltIn{};
    }
    Int op(const Int x, const Int y) const override { return x + y; }
};

struct Minus__BuiltIn : public Binary_Int_Int_Int_Op__BuiltIn {
    using Binary_Int_Int_Int_Op__BuiltIn::Op__BuiltIn;
    Binary_Int_Int_Int_Op__BuiltIn *clone() const override {
        return new Minus__BuiltIn{};
    }
    Int op(const Int x, const Int y) const override { return x - y; }
};

struct Multiply__BuiltIn : public Binary_Int_Int_Int_Op__BuiltIn {
    using Binary_Int_Int_Int_Op__BuiltIn::Op__BuiltIn;
    Binary_Int_Int_Int_Op__BuiltIn *clone() const override {
        return new Multiply__BuiltIn{};
    }
    Int op(const Int x, const Int y) const override { return x * y; }
};

struct Divide__BuiltIn : public Binary_Int_Int_Int_Op__BuiltIn {
    using Binary_Int_Int_Int_Op__BuiltIn::Op__BuiltIn;
    Binary_Int_Int_Int_Op__BuiltIn *clone() const override {
        return new Divide__BuiltIn{};
    }
    Int op(const Int x, const Int y) const override { return x / y; }
};

struct Exponentiate__BuiltIn : public Binary_Int_Int_Int_Op__BuiltIn {
    using Binary_Int_Int_Int_Op__BuiltIn::Op__BuiltIn;
    Binary_Int_Int_Int_Op__BuiltIn *clone() const override {
        return new Exponentiate__BuiltIn{};
    }
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
    using Binary_Int_Int_Int_Op__BuiltIn::Op__BuiltIn;
    Binary_Int_Int_Int_Op__BuiltIn *clone() const override {
        return new Modulo__BuiltIn{};
    }
    Int op(const Int x, const Int y) const override { return x % y; }
};

struct Left_Shift__BuiltIn : public Binary_Int_Int_Int_Op__BuiltIn {
    using Binary_Int_Int_Int_Op__BuiltIn::Op__BuiltIn;
    Binary_Int_Int_Int_Op__BuiltIn *clone() const override {
        return new Left_Shift__BuiltIn{};
    }
    Int op(const Int x, const Int y) const override { return x << y; }
};

struct Right_Shift__BuiltIn : public Binary_Int_Int_Int_Op__BuiltIn {
    using Binary_Int_Int_Int_Op__BuiltIn::Op__BuiltIn;
    Binary_Int_Int_Int_Op__BuiltIn *clone() const override {
        return new Right_Shift__BuiltIn{};
    }
    Int op(const Int x, const Int y) const override { return x >> y; }
};

struct Spaceship__BuiltIn : public Binary_Int_Int_Int_Op__BuiltIn {
    using Binary_Int_Int_Int_Op__BuiltIn::Op__BuiltIn;
    Binary_Int_Int_Int_Op__BuiltIn *clone() const override {
        return new Spaceship__BuiltIn{};
    }
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
    using Binary_Int_Int_Int_Op__BuiltIn::Op__BuiltIn;
    Binary_Int_Int_Int_Op__BuiltIn *clone() const override {
        return new Bitwise_And__BuiltIn{};
    }
    Int op(const Int x, const Int y) const override { return x & y; }
};

struct Bitwise_Or__BuiltIn : public Binary_Int_Int_Int_Op__BuiltIn {
    using Binary_Int_Int_Int_Op__BuiltIn::Op__BuiltIn;
    Binary_Int_Int_Int_Op__BuiltIn *clone() const override {
        return new Bitwise_Or__BuiltIn{};
    }
    Int op(const Int x, const Int y) const override { return x | y; }
};

struct Bitwise_Xor__BuiltIn : public Binary_Int_Int_Int_Op__BuiltIn {
    using Binary_Int_Int_Int_Op__BuiltIn::Op__BuiltIn;
    Binary_Int_Int_Int_Op__BuiltIn *clone() const override {
        return new Bitwise_Xor__BuiltIn{};
    }
    Int op(const Int x, const Int y) const override { return x ^ y; }
};

struct Increment__BuiltIn : public Unary_Int_Int_Op__BuiltIn {
    using Unary_Int_Int_Op__BuiltIn::Op__BuiltIn;
    Unary_Int_Int_Op__BuiltIn *clone() const override {
        return new Increment__BuiltIn{};
    }
    Int op(const Int x) const override { return x + 1; }
};

struct Decrement__BuiltIn : public Unary_Int_Int_Op__BuiltIn {
    using Unary_Int_Int_Op__BuiltIn::Op__BuiltIn;
    Unary_Int_Int_Op__BuiltIn *clone() const override {
        return new Decrement__BuiltIn{};
    }
    Int op(const Int x) const override { return x - 1; }
};

struct Negation__BuiltIn : public Unary_Bool_Bool_Op__BuiltIn {
    using Unary_Bool_Bool_Op__BuiltIn::Op__BuiltIn;
    Unary_Bool_Bool_Op__BuiltIn *clone() const override {
        return new Negation__BuiltIn{};
    }
    Bool op(const Bool x) const override { return !x; }
};

struct Comparison_LT__BuiltIn : public Binary_Int_Int_Bool_Op__BuiltIn {
    using Binary_Int_Int_Bool_Op__BuiltIn::Op__BuiltIn;
    Binary_Int_Int_Bool_Op__BuiltIn *clone() const override {
        return new Comparison_LT__BuiltIn{};
    }
    Bool op(const Int x, const Int y) const override { return x < y; }
};

struct Comparison_LE__BuiltIn : public Binary_Int_Int_Bool_Op__BuiltIn {
    using Binary_Int_Int_Bool_Op__BuiltIn::Op__BuiltIn;
    Binary_Int_Int_Bool_Op__BuiltIn *clone() const override {
        return new Comparison_LE__BuiltIn{};
    }
    Bool op(const Int x, const Int y) const override { return x <= y; }
};

struct Comparison_GT__BuiltIn : public Binary_Int_Int_Bool_Op__BuiltIn {
    using Binary_Int_Int_Bool_Op__BuiltIn::Op__BuiltIn;
    Binary_Int_Int_Bool_Op__BuiltIn *clone() const override {
        return new Comparison_GT__BuiltIn{};
    }
    Bool op(const Int x, const Int y) const override { return x > y; }
};

struct Comparison_GE__BuiltIn : public Binary_Int_Int_Bool_Op__BuiltIn {
    using Binary_Int_Int_Bool_Op__BuiltIn::Op__BuiltIn;
    Binary_Int_Int_Bool_Op__BuiltIn *clone() const override {
        return new Comparison_GE__BuiltIn{};
    }
    Bool op(const Int x, const Int y) const override { return x >= y; }
};

struct Comparison_EQ__BuiltIn : public Binary_Int_Int_Bool_Op__BuiltIn {
    using Binary_Int_Int_Bool_Op__BuiltIn::Op__BuiltIn;
    Binary_Int_Int_Bool_Op__BuiltIn *clone() const override {
        return new Comparison_EQ__BuiltIn{};
    }
    Bool op(const Int x, const Int y) const override { return x == y; }
};

struct Comparison_NE__BuiltIn : public Binary_Int_Int_Bool_Op__BuiltIn {
    using Binary_Int_Int_Bool_Op__BuiltIn::Op__BuiltIn;
    Binary_Int_Int_Bool_Op__BuiltIn *clone() const override {
        return new Comparison_NE__BuiltIn{};
    }
    Bool op(const Int x, const Int y) const override { return x != y; }
};
