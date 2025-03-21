#pragma once

#include "data_structures/lock.hpp"

#include <atomic>
#include <cassert>
#include <cstddef>
#include <optional>
#include <stdint.h>
#include <vector>

template <typename T> class CyclicQueue {
    std::vector<T> data;
    Locked<std::atomic<std::size_t>> front, back;
    std::size_t max_size_;
    T &get(std::size_t i) { return data[i % max_size_]; }

  public:
    CyclicQueue();
    CyclicQueue<T> &operator=(CyclicQueue<T> &&queue);
    explicit CyclicQueue(size_t max_size);
    size_t max_size() const;
    size_t size() const;
    bool empty() const;
    void push(const T &value);
    void push(T &&value);
    std::optional<T> pop();
};
