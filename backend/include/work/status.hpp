#pragma once

#include "data_structures/atomic_shared_enum.hpp"

#include <cstdint>
#include <memory>

class Status {
    enum WorkStatus { UNAVAILABLE, AVAILABLE, FULL, MAX };
    static inline constexpr std::size_t WORK_IDX = 0;
    static inline constexpr std::size_t WORK_WIDTH = 2;
    static_assert(MAX <= (1ULL << WORK_WIDTH));
    static inline constexpr std::size_t QUEUE_IDX = 1;
    static inline constexpr std::size_t QUEUE_WIDTH = 1;
    AtomicSharedEnum<WORK_WIDTH, QUEUE_WIDTH> value;

  public:
    Status() = default;
    bool queued() const { return value.load<QUEUE_IDX>(); }
    bool available() const { return value.load<WORK_IDX>() == AVAILABLE; }
    bool unavailable() const { return value.load<WORK_IDX>() == UNAVAILABLE; }
    bool full() const { return value.load<WORK_IDX>() == FULL; }

    bool request() {
        return value.compare_exchange<WORK_IDX>(UNAVAILABLE, AVAILABLE);
    }
    bool cancel() {
        return value.compare_exchange<WORK_IDX>(AVAILABLE, UNAVAILABLE);
    }
    bool fill() {
        if (value.compare_exchange<WORK_IDX, WORK_IDX>(AVAILABLE, FULL)) {
            value.store<QUEUE_IDX>(false);
            return true;
        } else {
            value.store<QUEUE_IDX>(false);
            return false;
        }
    }
    bool complete() {
        return value.compare_exchange<WORK_IDX>(FULL, UNAVAILABLE);
    }
    bool enqueue() {
        return value.compare_exchange<WORK_IDX, QUEUE_IDX, QUEUE_IDX>(
            AVAILABLE, false, true);
    }
    bool dequeue() {
        return value.compare_exchange<WORK_IDX, QUEUE_IDX>(UNAVAILABLE, false);
    }
};
