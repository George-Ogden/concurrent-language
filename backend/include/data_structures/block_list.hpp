#pragma once

#include <algorithm>
#include <cassert>
#include <cstddef>
#include <initializer_list>
#include <memory>
#include <utility>

template <typename T> class BlockList {
    struct Block {
        const size_t length;
        T *data;
        Block *next, *prev;
        explicit Block(std::size_t length) : Block(new T[length], length){};
        explicit Block(T *data, std::size_t length)
            : length(length), data(data), next(nullptr), prev(nullptr){};
        ~Block() { delete[] data; }
        Block(const Block &other) = delete;
        Block &operator=(const Block &other) = delete;
    };
    template <typename U> class Iterator {
        friend BlockList;
        Block *block;
        U *pointer;

      protected:
        bool at_end_of_block() const {
            return pointer == nullptr || pointer == block->data + block->length;
        }
        bool at_start_of_block() const { return pointer == block->data; }
        void maybe_jump_blocks_dangerous() {
            if (at_end_of_block()) {
                jump_blocks();
            }
        }
        void maybe_jump_blocks_safe() {
            if (block->next != nullptr) {
                maybe_jump_blocks_dangerous();
            }
        }
        void jump_blocks() {
            block = block->next;
            pointer = block->data;
        }
        void maybe_jump_back_blocks_dangerous() {
            if (at_start_of_block()) {
                block = block->prev;
                pointer = block->data + block->length;
            }
        }

        explicit Iterator(Block *block) : Iterator(block, block->data) {}
        Iterator(Block *block, U *pointer) : block(block), pointer(pointer) {}

      public:
        Iterator() : Iterator(nullptr, nullptr) {}
        using iterator_category = std::random_access_iterator_tag;
        using difference_type = std::ptrdiff_t;
        using value_type = U;
        using reference = value_type &;
        Iterator &operator++() {
            maybe_jump_blocks_dangerous();
            pointer++;
            maybe_jump_blocks_safe();
            return *this;
        }
        Iterator operator++(int) {
            auto tmp = *this;
            ++*this;
            return tmp;
        }
        Iterator &operator--() {
            maybe_jump_back_blocks_dangerous();
            pointer--;
            return *this;
        }
        Iterator operator--(int) {
            auto tmp = *this;
            --*this;
            return tmp;
        }
        Iterator operator+(difference_type distance) const {
            auto tmp = *this;
            tmp += distance;
            return tmp;
        }
        friend Iterator operator+(difference_type distance,
                                  const Iterator &it) {
            return it + distance;
        }
        Iterator &operator+=(difference_type distance) {
            if (distance > 0) {
                while (distance > 0) {
                    maybe_jump_blocks_dangerous();
                    difference_type n =
                        std::min(distance, distance_remaining());
                    pointer += n;
                    distance -= n;
                }
            } else {
                distance *= -1;
                while (distance > 0) {
                    maybe_jump_back_blocks_dangerous();
                    difference_type n = std::min(distance, distance_along());
                    pointer -= n;
                    distance -= n;
                }
            }
            return *this;
        }
        reference operator*() {
            maybe_jump_blocks_dangerous();
            return *pointer;
        }
        reference operator*() const {
            Iterator copy = *this;
            return *copy;
        }
        bool operator==(const Iterator &other) const {
            return ConstIterator(*this) == ConstIterator(other);
        }

        difference_type distance_along() const {
            return std::distance(block->data, pointer);
        }
        difference_type distance_remaining() const {
            if (at_end_of_block())
                return 0;
            return std::distance(pointer, block->data + block->length);
        }
    };
    template <typename U> struct ConstIterator : public Iterator<const U> {
        // cppcheck-suppress noExplicitConstructor
        ConstIterator(const Iterator<U> &it)
            : Iterator<const U>(it.block, it.pointer){};
        bool operator==(const Iterator<U> &other) const {
            return *this == ConstIterator(other);
        }
        bool operator==(const ConstIterator<U> &other) const {
            ConstIterator<U> left = *this;
            ConstIterator<U> right = other;
            left.maybe_jump_blocks_safe();
            right.maybe_jump_blocks_safe();
            return left.block == right.block && left.pointer == right.pointer;
        }
    };
    Block *head;
    Iterator<T> _begin, _end;
    size_t _size;

  protected:
    void add_block(Block *block) {
        _end.block->next = block;
        block->prev = _end.block;
        _end.jump_blocks();
    }

  public:
    using value_type = T;
    using reference = value_type &;
    using const_reference = const reference;
    using iterator = Iterator<T>;
    using const_iterator = ConstIterator<T>;
    using size_type = size_t;
    static constexpr std::size_t compute_length(std::size_t size) {
        return std::max(static_cast<size_t>(16), size / sizeof(T));
    }
    BlockList()
        : head(new Block(new T[0], 0)), _begin(head, nullptr),
          _end(head, nullptr), _size(0) {}
    explicit BlockList(std::initializer_list<T> init) : BlockList() {
        append_range(init);
    }
    ~BlockList() {
        Block *next = head;
        while (next != nullptr) {
            Block *block = next;
            next = block->next;
            delete block;
        }
    }
    BlockList(const BlockList &other) = delete;
    BlockList &operator=(const BlockList &other) = delete;
    size_type size() const { return _size; }
    iterator begin() { return _begin; }
    iterator end() { return _end; }
    const_iterator begin() const { return _begin; }
    const_iterator end() const { return _end; }
    const_iterator cbegin() const { return _begin; }
    const_iterator cend() const { return _end; }
    reference front() { return *begin(); }
    const_reference front() const { return *begin(); }
    reference back() { return *std::prev(end()); }
    const_reference back() const { return *std::prev(end()); }
    void push_back(T &&value) { emplace_back(std::move(value)); }
    void push_back(const T &value) { emplace_back(value); }
    template <typename... Args> reference emplace_back(Args &&...args) {
        if (_end.at_end_of_block()) {
            add_block(
                new Block(std::max(_end.block->length, compute_length(1024))));
        }
        // cppcheck-suppress constVariable
        reference &ref =
            *std::construct_at(_end.pointer, std::forward<Args>(args)...);
        ++_end;
        _size++;
        return ref;
    }
    void pop_front() {
        ++_begin;
        if (head->next != _begin.block) {
            head = head->next;
            delete head->prev;
            head->prev = nullptr;
        }
        _size--;
    }
    void pop_back() {
        // Only keep one additional block.
        if (_end.at_start_of_block() && _end.block->next != nullptr) {
            delete _end.block->next;
            _end.block->next = nullptr;
        }
        --_end;
        _size--;
    }
    bool empty() const { return begin() == end(); }
    template <typename R> constexpr void append_range(R &&rg) {
        auto it = rg.begin();
        {
            size_t n = std::min(rg.size(),
                                static_cast<size_t>(_end.distance_remaining()));
            std::copy_n(it, n, _end.pointer);
            _size += n;
        }
        if (it != rg.end()) {
            size_t n = static_cast<size_t>(std::distance(it, rg.end()));
            Block *block = new Block(n);

            std::copy_n(it, n, block->data);
            add_block(block);

            std::advance(it, n);
            std::advance(_end, n);
            _size += n;
        }
    }
};
