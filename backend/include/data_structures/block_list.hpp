#pragma once

#include <algorithm>
#include <cstddef>
#include <optional>

template <typename T> class BlockList {
  private:
    struct Block {
        const size_t length;
        T *data;
        Block *next, *prev;

        explicit Block(std::size_t length);
        explicit Block(T *data, std::size_t length);
        ~Block();

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
        using iterator_category = std::bidirectional_iterator_tag;
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
            return ConstIterator<U>(*this) == ConstIterator<U>(other);
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
    std::size_t _size;

  protected:
    void add_block(Block *block);

  public:
    using value_type = T;
    using reference = value_type &;
    using const_reference = value_type const &;
    using iterator = Iterator<T>;
    using const_iterator = ConstIterator<T>;
    using size_type = size_t;

    static constexpr std::size_t compute_length(std::size_t size);

    BlockList();
    ~BlockList();
    BlockList(const BlockList &other) = delete;
    BlockList &operator=(const BlockList &other) = delete;

    size_type size() const;
    iterator begin();
    iterator end();
    const_iterator begin() const;
    const_iterator end() const;
    const_iterator cbegin() const;
    const_iterator cend() const;
    reference front();
    const_reference front() const;
    reference back();
    const_reference back() const;
    void push_back(T &&value);
    void push_back(const T &value);
    void clear();

    template <typename... Args> reference emplace_back(Args &&...args);

    std::optional<T> pop_back();
    bool empty() const;
};
