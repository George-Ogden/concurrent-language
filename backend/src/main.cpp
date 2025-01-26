#include "main/main.hpp"
#include "system/work_manager.hpp"

#include <chrono>
#include <iostream>

int main() {
    auto start = std::chrono::high_resolution_clock::now();

    Main main{};
    WorkManager::run(&main);

    auto end = std::chrono::high_resolution_clock::now();

    std::cout << main.value() << std::endl;

    auto duration =
        std::chrono::duration_cast<std::chrono::nanoseconds>(end - start);
    std::cerr << "Execution time: " << duration.count() << " ns" << std::endl;

    return 0;
}
