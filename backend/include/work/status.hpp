#pragma once

struct Status {
    enum Value {
        available,
        queued,
        // required,
        // active,
        finished
    };
    // cppcheck-suppress noExplicitConstructor
    Status(Value status) : value(status){};
    bool done() const { return *this == finished; }
    friend bool operator==(const Status &lhs, const Value &rhs) {
        return lhs.value == rhs;
    }
    friend bool operator==(const Value &lhs, const Status &rhs) {
        return lhs == rhs.value;
    }

  private:
    Value value;
};
