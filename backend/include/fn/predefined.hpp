#pragma once

#include "fn/fn.hpp"
#include "types/predefined.hpp"

struct Plus__BuiltIn : public ParametricFn<Int, Int, Int> {
    void body() override { *ret = *std::get<0>(args) + *std::get<1>(args); }
};

struct Minus__BuiltIn : public ParametricFn<Int, Int, Int> {
    void body() override { *ret = *std::get<0>(args) - *std::get<1>(args); }
};
