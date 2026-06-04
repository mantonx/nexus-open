#!/bin/bash
# Zone-Based Configuration Testing Script
# Tests the per-zone and per-module configuration system

set -e  # Exit on error

API_URL="http://localhost:1985"
CONFIG_FILE="$HOME/.config/nexus-open/zone-configs.yaml"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Test counter
TESTS_PASSED=0
TESTS_FAILED=0

print_header() {
    echo -e "\n${BLUE}========================================${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}========================================${NC}\n"
}

print_test() {
    echo -e "${YELLOW}TEST:${NC} $1"
}

print_success() {
    echo -e "${GREEN}✓ PASS:${NC} $1"
    ((TESTS_PASSED++))
}

print_failure() {
    echo -e "${RED}✗ FAIL:${NC} $1"
    ((TESTS_FAILED++))
}

print_info() {
    echo -e "${BLUE}ℹ INFO:${NC} $1"
}

# Check if server is running
check_server() {
    print_test "Checking if API server is running"
    if curl -s "$API_URL/api/health" > /dev/null 2>&1; then
        print_success "API server is running at $API_URL"
        return 0
    else
        print_failure "API server is not running at $API_URL"
        echo "Please start the server with: ./bin/nexus-open"
        exit 1
    fi
}

# Test 1: Get module default (should be empty initially)
test_get_module_default_empty() {
    print_test "GET module default config (should be empty initially)"
    
    RESPONSE=$(curl -s "$API_URL/api/modules/weather/config")
    
    if echo "$RESPONSE" | grep -q '"config":null\|"config":{}'; then
        print_success "Module default is empty (as expected)"
    else
        print_info "Response: $RESPONSE"
    fi
}

# Test 2: Set module default configuration
test_set_module_default() {
    print_test "POST module default config for weather module"
    
    RESPONSE=$(curl -s -X POST "$API_URL/api/modules/weather/config" \
        -H "Content-Type: application/json" \
        -d '{"location":"New York, NY","unit":"imperial"}')
    
    if echo "$RESPONSE" | grep -q '"status":"success"'; then
        print_success "Module default config set successfully"
    else
        print_failure "Failed to set module default config"
        print_info "Response: $RESPONSE"
    fi
}

# Test 3: Get module default (should return what we set)
test_get_module_default() {
    print_test "GET module default config (verify it was saved)"
    
    RESPONSE=$(curl -s "$API_URL/api/modules/weather/config")
    
    if echo "$RESPONSE" | grep -q '"location":"New York, NY"' && \
       echo "$RESPONSE" | grep -q '"unit":"imperial"'; then
        print_success "Module default config retrieved correctly"
    else
        print_failure "Module default config not retrieved correctly"
        print_info "Response: $RESPONSE"
    fi
}

# Test 4: Set zone override configuration
test_set_zone_override() {
    print_test "POST zone override config for weather zone"
    
    RESPONSE=$(curl -s -X POST "$API_URL/api/zones/weather/config" \
        -H "Content-Type: application/json" \
        -d '{"location":"San Francisco, CA","unit":"metric"}')
    
    if echo "$RESPONSE" | grep -q '"status":"success"'; then
        print_success "Zone override config set successfully"
    else
        print_failure "Failed to set zone override config"
        print_info "Response: $RESPONSE"
    fi
}

# Test 5: Get zone override (should return override, not default)
test_get_zone_override() {
    print_test "GET zone override config (verify it overrides default)"
    
    RESPONSE=$(curl -s "$API_URL/api/zones/weather/config")
    
    if echo "$RESPONSE" | grep -q '"location":"San Francisco, CA"' && \
       echo "$RESPONSE" | grep -q '"unit":"metric"'; then
        print_success "Zone override config retrieved correctly"
    else
        print_failure "Zone override config not retrieved correctly"
        print_info "Response: $RESPONSE"
    fi
}

# Test 6: Set CPU temperature zone config
test_set_cpu_config() {
    print_test "POST zone config for CPU temperature (Celsius)"
    
    RESPONSE=$(curl -s -X POST "$API_URL/api/zones/cpu/config" \
        -H "Content-Type: application/json" \
        -d '{"unit":"metric"}')
    
    if echo "$RESPONSE" | grep -q '"status":"success"'; then
        print_success "CPU zone config set to Celsius"
    else
        print_failure "Failed to set CPU zone config"
        print_info "Response: $RESPONSE"
    fi
}

# Test 7: Set GPU temperature zone config (different from CPU)
test_set_gpu_config() {
    print_test "POST zone config for GPU temperature (Fahrenheit)"
    
    RESPONSE=$(curl -s -X POST "$API_URL/api/zones/gpu/config" \
        -H "Content-Type: application/json" \
        -d '{"unit":"imperial"}')
    
    if echo "$RESPONSE" | grep -q '"status":"success"'; then
        print_success "GPU zone config set to Fahrenheit"
    else
        print_failure "Failed to set GPU zone config"
        print_info "Response: $RESPONSE"
    fi
}

# Test 8: Verify zone configs are different
test_verify_different_configs() {
    print_test "Verify CPU and GPU have different temperature units"
    
    CPU_RESPONSE=$(curl -s "$API_URL/api/zones/cpu/config")
    GPU_RESPONSE=$(curl -s "$API_URL/api/zones/gpu/config")
    
    if echo "$CPU_RESPONSE" | grep -q '"unit":"metric"' && \
       echo "$GPU_RESPONSE" | grep -q '"unit":"imperial"'; then
        print_success "CPU (metric) and GPU (imperial) configs are different"
    else
        print_failure "Zone configs are not different as expected"
        print_info "CPU Response: $CPU_RESPONSE"
        print_info "GPU Response: $GPU_RESPONSE"
    fi
}

# Test 9: Check if zone-configs.yaml file exists and has content
test_config_file_persistence() {
    print_test "Check if zone-configs.yaml file exists and has content"
    
    if [ -f "$CONFIG_FILE" ]; then
        print_success "Zone config file exists at $CONFIG_FILE"
        print_info "File contents:"
        cat "$CONFIG_FILE" | sed 's/^/    /'
        
        # Check if file has expected content
        if grep -q "zone_overrides:" "$CONFIG_FILE" || grep -q "module_defaults:" "$CONFIG_FILE"; then
            print_success "Config file contains expected structure"
        else
            print_failure "Config file doesn't contain expected structure"
        fi
    else
        print_failure "Zone config file not found at $CONFIG_FILE"
    fi
}

# Test 10: Delete zone override
test_delete_zone_override() {
    print_test "DELETE zone override for weather zone"
    
    RESPONSE=$(curl -s -X DELETE "$API_URL/api/zones/weather/config")
    
    if echo "$RESPONSE" | grep -q '"status":"success"'; then
        print_success "Zone override deleted successfully"
    else
        print_failure "Failed to delete zone override"
        print_info "Response: $RESPONSE"
    fi
}

# Test 11: Verify override was deleted (should fall back to default)
test_verify_override_deleted() {
    print_test "GET zone config after delete (should be empty or fall back to default)"
    
    RESPONSE=$(curl -s "$API_URL/api/zones/weather/config")
    
    # After deletion, the API should return empty config or the response should be empty
    if echo "$RESPONSE" | grep -q '"config":null\|"config":{}'; then
        print_success "Zone override was deleted (config is empty)"
    else
        print_info "Response: $RESPONSE"
        print_info "Note: Zone might still have some config from defaults"
    fi
}

# Test 12: Test invalid JSON
test_invalid_json() {
    print_test "POST invalid JSON (should return error)"
    
    RESPONSE=$(curl -s -X POST "$API_URL/api/zones/cpu/config" \
        -H "Content-Type: application/json" \
        -d 'invalid json')
    
    if echo "$RESPONSE" | grep -qi 'error\|invalid'; then
        print_success "Server correctly rejected invalid JSON"
    else
        print_failure "Server did not reject invalid JSON"
        print_info "Response: $RESPONSE"
    fi
}

# Test 13: Test non-existent zone
test_nonexistent_zone() {
    print_test "POST config to non-existent zone (should succeed - zones are dynamic)"
    
    RESPONSE=$(curl -s -X POST "$API_URL/api/zones/nonexistent/config" \
        -H "Content-Type: application/json" \
        -d '{"test":"value"}')
    
    # Zone configs can be set even if zone doesn't exist yet
    if echo "$RESPONSE" | grep -q '"status":"success"'; then
        print_success "Config saved for non-existent zone (will apply when zone is created)"
    else
        print_info "Response: $RESPONSE"
    fi
}

# Print summary
print_summary() {
    print_header "TEST SUMMARY"
    TOTAL=$((TESTS_PASSED + TESTS_FAILED))
    echo "Total Tests: $TOTAL"
    echo -e "${GREEN}Passed: $TESTS_PASSED${NC}"
    echo -e "${RED}Failed: $TESTS_FAILED${NC}"
    
    if [ $TESTS_FAILED -eq 0 ]; then
        echo -e "\n${GREEN}All tests passed! ✓${NC}\n"
        return 0
    else
        echo -e "\n${RED}Some tests failed! ✗${NC}\n"
        return 1
    fi
}

# Main test execution
main() {
    print_header "Zone-Based Configuration System Tests"
    
    check_server
    
    print_header "Module Default Configuration Tests"
    test_get_module_default_empty
    test_set_module_default
    test_get_module_default
    
    print_header "Zone Override Configuration Tests"
    test_set_zone_override
    test_get_zone_override
    
    print_header "Multiple Zone Configuration Tests"
    test_set_cpu_config
    test_set_gpu_config
    test_verify_different_configs
    
    print_header "Persistence Tests"
    test_config_file_persistence
    
    print_header "Delete Operation Tests"
    test_delete_zone_override
    test_verify_override_deleted
    
    print_header "Error Handling Tests"
    test_invalid_json
    test_nonexistent_zone
    
    print_summary
}

# Run main function
main
exit $?
