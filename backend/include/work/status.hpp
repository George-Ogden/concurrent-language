#pragma once

#include "data_structures/atomic_shared_enum.hpp"

#include <cstdint>
#include <memory>

class Status {
    static inline constexpr std::size_t QUEUED_IDX = 0;
    static inline constexpr std::size_t QUEUED_WIDTH = 1;
    static inline constexpr std::size_t EXECUTION_IDX = 1;
    static inline constexpr std::size_t EXECUTION_WIDTH = 2;
    static inline constexpr std::size_t REQUIRED_IDX = 2;
    static inline constexpr std::size_t REQUIRED_WIDTH = 1;
    AtomicSharedEnum<QUEUED_WIDTH, EXECUTION_WIDTH, REQUIRED_WIDTH> value;

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
    bool required() const { return value.load<REQUIRED_IDX>(); }
    bool require() {
        return value.compare_exchange<REQUIRED_IDX>(
                   false, true, std::memory_order_release) &&
               !done();
    }
};
