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
    template <std::size_t section> constexpr uint8_t mask() const {
        return ((1ULL << widths[section]) - 1) << prefix_widths[section];
    }
    uint8_t value() const { return bits.load(std::memory_order_relaxed); }
    template <std::size_t section> uint8_t insert(const uint8_t &value) {
        return ((~mask<section>()) & this->value()) |
               (mask<section>() & (value << prefix_widths[section]));
    }

  public:
    AtomicSharedEnum() : bits(0){};
    template <std::size_t section>
    requires(section < sizeof...(Widths)) uint8_t
        load(std::memory_order ordering = std::memory_order_relaxed)
    const {
        return (bits.load(ordering) >> prefix_widths[section]) &
               ((1ULL << widths[section]) - 1);
    }
    template <std::size_t section>
    requires(section < sizeof...(Widths) && widths[section] == 1) bool flip(
        std::memory_order ordering = std::memory_order_relaxed) {
        return bits.fetch_xor(1 << prefix_widths[section], ordering) >>
               prefix_widths[section];
    }
    template <std::size_t section>
    requires(section < sizeof...(Widths)) void store(
        uint8_t value, std::memory_order ordering = std::memory_order_relaxed) {
        exchange<section>(value, ordering);
    }
    template <std::size_t compare_section,
              std::size_t exchange_section = compare_section>
    requires(
        compare_section < sizeof...(Widths) &&
        exchange_section <
            sizeof...(
                Widths)) bool compare_exchange(uint8_t expected,
                                               uint8_t desired,
                                               std::memory_order ordering =
                                                   std::memory_order_relaxed) {
        while (1) {
            uint8_t current_value = value();
            uint8_t expected_value = insert<compare_section>(expected);
            uint8_t desired_value = insert<exchange_section>(desired);
            if (bits.compare_exchange_weak(expected_value, desired_value,
                                           ordering,
                                           std::memory_order_relaxed)) {
                return true;
                // cppcheck-suppress knownConditionTrueFalse
            } else if (value() != current_value) {
                continue;
            } else {
                return false;
            }
        };
    }
    template <std::size_t section>
    requires(section < sizeof...(Widths)) uint8_t
        exchange(uint8_t value,
                 std::memory_order ordering = std::memory_order_relaxed) {
        uint8_t current_value, desired;
        do {
            current_value = this->value();
            desired = insert<section>(value);
        } while (!bits.compare_exchange_weak(current_value, desired, ordering,
                                             std::memory_order_relaxed));
        return (current_value & mask<section>()) >> prefix_widths[section];
    }
};
