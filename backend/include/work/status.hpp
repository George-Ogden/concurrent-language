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
    template <std::size_t section>
    requires(section < sizeof...(Widths)) bool compare_exchange(
        uint8_t expected, uint8_t desired,
        std::memory_order ordering = std::memory_order_relaxed) {
        uint8_t mask = ((1ULL << widths[section]) - 1)
                       << prefix_widths[section];
        while (1) {
            uint8_t current_value = bits.load(std::memory_order_relaxed);
            uint8_t expected_value = ((~mask) & current_value) |
                                     (expected << prefix_widths[section]);
            uint8_t desired_value =
                ((~mask) & current_value) | (desired << prefix_widths[section]);
            if (bits.compare_exchange_weak(expected_value, desired_value,
                                           ordering)) {
                return true;
            } else if (bits.load(std::memory_order_relaxed) != current_value) {
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
        uint8_t mask = ((1ULL << widths[section]) - 1)
                       << prefix_widths[section];
        uint8_t current_value, desired;
        do {
            current_value = bits.load(std::memory_order_relaxed);
            desired =
                ((~mask) & current_value) | (value << prefix_widths[section]);
        } while (!bits.compare_exchange_weak(current_value, desired, ordering));
        return (current_value & mask) >> prefix_widths[section];
    }
};

class Status {
    static inline constexpr std::size_t QUEUED_IDX = 0;
    static inline constexpr std::size_t QUEUED_WIDTH = 1;
    static inline constexpr std::size_t EXECUTION_IDX = 1;
    static inline constexpr std::size_t EXECUTION_WIDTH = 2;
    AtomicSharedEnum<QUEUED_WIDTH, EXECUTION_WIDTH> value;

  public:
    enum ExecutionStatus { available = 0, active, finished, MAX };
    static_assert(ExecutionStatus::MAX <= (1 << EXECUTION_WIDTH));
    Status() : value(){};
    ExecutionStatus execution_status() const {
        return static_cast<ExecutionStatus>(value.load<EXECUTION_IDX>());
    }
    bool done() const {
        return value.load<EXECUTION_IDX>() == ExecutionStatus::finished;
    }
    bool start_work() {
        return value.compare_exchange<EXECUTION_IDX>(ExecutionStatus::available,
                                                     ExecutionStatus::active,
                                                     std::memory_order_acquire);
    }
    bool cancel_work() {
        return value.compare_exchange<EXECUTION_IDX>(ExecutionStatus::active,
                                                     ExecutionStatus::available,
                                                     std::memory_order_release);
    }
    void finish_work() {
        value.store<EXECUTION_IDX>(ExecutionStatus::finished,
                                   std::memory_order_release);
    }
    bool queued() const { return value.load<QUEUED_IDX>(); }
    bool enqueue() {
        if (execution_status() != ExecutionStatus::available) {
            return false;
        }
        return value.compare_exchange<QUEUED_IDX>(false, true,
                                                  std::memory_order_acq_rel);
    }
    bool dequeue() {
        return value.compare_exchange<QUEUED_IDX>(true, false,
                                                  std::memory_order_acq_rel) &&
               execution_status() == ExecutionStatus::available;
    }
};
