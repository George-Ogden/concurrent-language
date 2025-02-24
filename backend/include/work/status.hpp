#pragma once

#include "data_structures/atomic_shared_enum.hpp"

#include <cstdint>
#include <memory>

class Status {
    static inline constexpr std::size_t OWNED_IDX = 0;
    static inline constexpr std::size_t OWNED_WIDTH = 1;
    static inline constexpr std::size_t REQUIRED_IDX = 1;
    static inline constexpr std::size_t REQUIRED_WIDTH = 1;
    static inline constexpr std::size_t DONE_IDX = 2;
    static inline constexpr std::size_t DONE_WIDTH = 1;
    AtomicSharedEnum<OWNED_WIDTH, REQUIRED_WIDTH, DONE_WIDTH> value;

  public:
    Status();
    bool acquire();
    bool release();
    bool require();
    bool required() const;
    void finish();
    bool done() const;
};
