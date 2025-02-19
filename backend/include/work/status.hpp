#pragma once

class Status {
  public:
    enum Value {
        available,
        // queued,
        // required,
        // active,
        finished
    };
    Status() = default;
    // cppcheck-suppress noExplicitConstructor
    Status(Value status) : value(status){};
    bool done() const { return value == finished; }

  private:
    Value value = available;
};
