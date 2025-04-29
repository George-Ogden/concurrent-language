#pragma once

#include "work/finished.hpp"

struct finished : public std::exception {};

void FinishedWork::run() {
    throw finished{};
};

void FinishedWork::await_all() {};

bool FinishedWork::can_respond() const {
    return true;
};

bool FinishedWork::execute_immediately() const {
    return true;
}
