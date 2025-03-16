#pragma once

#include "data_structures/cyclic_queue.hpp"
#include "data_structures/lock.tpp"

#include <atomic>
#include <cassert>
#include <cstddef>
#include <optional>
#include <stdint.h>
#include <vector>

template <typename T>
CyclicQueue<T>::CyclicQueue() : CyclicQueue(0) {}

template <typename T>
CyclicQueue<T> &CyclicQueue<T>::operator=(CyclicQueue<T> &&queue) {
    assert(*queue.front == 0);
    assert(*queue.back == 0);
    *front = 0;
    *back = 0;
    max_size_ = queue.max_size_;
    data = queue.data;
    return *this;
}

template <typename T>
CyclicQueue<T>::CyclicQueue(size_t max_size)
    : data(max_size), front(0), back(0), max_size_(max_size) {}

    template <typename T>
size_t CyclicQueue<T>::max_size() const { return max_size_; }

template <typename T>
size_t CyclicQueue<T>::size() const {
    return *back - *front;
}

template <typename T>
bool CyclicQueue<T>::empty() const {
    return *back == *front;
}

template <typename T>
T& CyclicQueue<T>::push(const T &value) {
    back.acquire();
    get(*back) = value;
    auto &ref = get(*back);
    (*back)++;
    back.release();
    return ref;
}

template <typename T>
T& CyclicQueue<T>::push(T &&value) {
    back.acquire();
    get(*back) = std::move(value);
    auto &ref = get(*back);
    (*back)++;
    back.release();
    return ref;
}

template <typename T>
std::optional<T> CyclicQueue<T>::pop() {
    front.acquire();
    if (*front == *back){
        front.release();
        return std::nullopt;
    }
    T &data = get((*front)++);
    front.release();
    T ret = data;
    data = T{};
    return ret;
}
