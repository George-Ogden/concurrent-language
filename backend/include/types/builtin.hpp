#pragma once

#include <cstdint>
#include <iostream>

typedef int64_t Int;
typedef bool Bool;

typedef std::monostate Empty;

std::ostream &operator<<(std::ostream &os, Empty const &e) {
    os << "()";
    return os;
}
