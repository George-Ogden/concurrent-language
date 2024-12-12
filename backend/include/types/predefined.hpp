#pragma once

#include <cstdint>
#include <type_traits>

typedef int64_t Int;
typedef bool Bool;
template <typename... Ts> using Tuple = std::tuple<Ts...>;
