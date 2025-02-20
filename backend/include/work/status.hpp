#pragma once

#include <algorithm>
#include <array>
#include <atomic>
#include <cstdint>
#include <memory>
#include <type_traits>
#include <utility>

template <std::size_t... Widths> struct prefix_sum {
  private:
    static constexpr std::array<std::size_t, sizeof...(Widths)> widths = {
        Widths...};

    template <std::size_t N, std::size_t... Is>
    static constexpr auto calculate_impl(std::index_sequence<Is...>) {
        std::array<std::size_t, N + 1> result = {0};
        ((result[Is + 1] = result[Is] + widths[Is]), ...);
        return result;
    }

  public:
    static constexpr auto value = calculate_impl<sizeof...(Widths)>(
        std::make_index_sequence<sizeof...(Widths)>{});
};

template <std::size_t... Widths>
inline constexpr auto prefix_sum_v = prefix_sum<Widths...>::value;

template <std::size_t... Widths> class AtomicSharedEnum {
    static_assert(std::max({Widths...}) <= 8);
    static_assert((Widths + ...) <= 8);
    std::atomic<uint8_t> bits;
    static constexpr inline auto widths =
        std::array<std::size_t, sizeof...(Widths)>{Widths...};
    static constexpr inline auto prefix_widths = prefix_sum_v<Widths...>;

  public:
    AtomicSharedEnum() : bits(0){};
    template <std::size_t section>
    requires(section < sizeof...(Widths) && widths[section] == 1) bool flip(
        std::memory_order ordering = std::memory_order_relaxed) {
        return bits.fetch_xor(1 << prefix_widths[section], ordering) >>
               prefix_widths[section];
    }
    template <std::size_t section>
    requires(section < sizeof...(Widths)) uint8_t
        get(std::memory_order ordering = std::memory_order_relaxed)
    const {
        return (bits.load(ordering) >> prefix_widths[section]) &
               ((1ULL << widths[section]) - 1);
    }
};

struct Status {
    enum Value {
        available,
        queued,
        // required,
        active,
        finished
    };
    // cppcheck-suppress noExplicitConstructor
    Status(Value status) : value(status){};
    bool done() const { return *this == finished; }
    friend bool operator==(const Status &lhs, const Value &rhs) {
        return lhs.value == rhs;
    }
    friend bool operator==(const Value &lhs, const Status &rhs) {
        return lhs == rhs.value;
    }
    Value operator*() const { return value; }

  private:
    Value value;
};
