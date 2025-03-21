#pragma once

#include "data_structures/block_list.hpp"
#include "lazy/types.hpp"
#include "lazy/lazy.tpp"

template <typename T>
BlockList<T>::Block::Block(std::size_t length)
    : Block(new T[length], length) {}

template <typename T>
BlockList<T>::Block::Block(T* data, std::size_t length)
    : length(length), data(data), next(nullptr), prev(nullptr) {}

template <typename T>
BlockList<T>::Block::~Block() {
    delete[] data;
}

template <typename T>
void BlockList<T>::add_block(Block *block) {
    _end.block->next = block;
    block->prev = _end.block;
    _end.jump_blocks();
}

template <typename T>
constexpr std::size_t BlockList<T>::compute_length(std::size_t size) {
    return std::max(static_cast<size_t>(16), size / sizeof(T));
}

template <typename T>
BlockList<T>::BlockList()
    : head(new Block{0}), _begin(head, nullptr), _end(head, nullptr), _size(0) {
}

template <typename T>
BlockList<T>::~BlockList() {
    Block *next = head;
    while (next != nullptr) {
        Block *block = next;
        next = block->next;
        delete block;
    }
}

template <typename T>
typename BlockList<T>::size_type BlockList<T>::size() const {
    return _size;
}
template <typename T>
typename BlockList<T>::iterator BlockList<T>::begin() { return _begin; }
template <typename T>
typename BlockList<T>::iterator BlockList<T>::end() { return _end; }
template <typename T>
typename BlockList<T>::const_iterator BlockList<T>::begin() const { return _begin; }
template <typename T>
typename BlockList<T>::const_iterator BlockList<T>::end() const { return _end; }
template <typename T>
typename BlockList<T>::const_iterator BlockList<T>::cbegin() const { return _begin; }
template <typename T>
typename BlockList<T>::const_iterator BlockList<T>::cend() const { return _end; }
template <typename T>
typename BlockList<T>::reference BlockList<T>::front() { return *begin(); }
template <typename T>
typename BlockList<T>::const_reference BlockList<T>::front() const { return *begin(); }
template <typename T>
typename BlockList<T>::reference BlockList<T>::back() { return *std::prev(end()); }
template <typename T>
typename BlockList<T>::const_reference BlockList<T>::back() const { return *std::prev(end()); }
template <typename T>
void BlockList<T>::push_back(T &&value) { emplace_back(std::move(value)); }
template <typename T>
void BlockList<T>::push_back(const T &value) { emplace_back(value); }


template <typename T>
void BlockList<T>::clear() {
    Block *next = head->next;
    while (next != nullptr) {
        Block *block = next;
        next = block->next;
        delete block;
    }
    head->next = nullptr;
    _begin.block = head;
    _begin.pointer = head->data;
    _end = _begin;
    _size = 0;
}

template <typename T>
template <typename... Args>
typename BlockList<T>::reference BlockList<T>::emplace_back(Args &&...args) {
    if (_end.at_end_of_block()) {
        if (_end.block->next == nullptr) {
            add_block(new Block(std::max(_end.block->length, compute_length(1024))));
        } else {
            _end.jump_blocks();
        }
    }
    reference &ref = *std::construct_at(_end.pointer, std::forward<Args>(args)...);
    ++_end;
    _size++;
    return ref;
}

template <typename T>
    std::optional<T> BlockList<T>::pop_back() {
        // Only keep one additional block.
        if (empty()) {
            return std::nullopt;
        }
        if (_end.at_start_of_block() && _end.block->next != nullptr) {
            delete _end.block->next;
            _end.block->next = nullptr;
        }
        T last = *std::prev(end());
        if constexpr (is_shared_ptr_v<T>){
            (*std::prev(end())).reset();
        }
        --_end;
        _size--;
        return last;
    }

template <typename T>
    bool BlockList<T>::empty() const { return begin() == end(); }
