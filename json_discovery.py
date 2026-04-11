"""JSON parser and server discovery for BSCP"""

import requests
import json
import time

# In-memory cache: key = (domain, server_type), value = (config, timestamp)
_discovery_cache = {}
_CACHE_TTL = 60  # seconds


def parse_config(json_text: str) -> dict:
    """Parse JSON config into a Python dictionary with basic error handling."""
    if not json_text or not json_text.strip():
        return {}

    try:
        return json.loads(json_text)
    except json.JSONDecodeError as e:
        print(f"JSON Parse Error: {e}")
        print("First 300 characters received:")
        print(repr(json_text[:300]))
        return {}
    except Exception as e:
        print(f"Unexpected error while parsing JSON: {e}")
        return {}


def discover_server(domain: str, server_type: str = "userserver"):
    """Fetch and parse JSON config from .well-known endpoint (cached for 60s)"""
    cache_key = (domain, server_type)
    cached = _discovery_cache.get(cache_key)
    if cached:
        config, ts = cached
        if time.time() - ts < _CACHE_TTL:
            return config

    try:
        url = f"http://{domain}/.well-known/BSCP/{server_type}.json"
        print(f"Fetching: {url}")  # for debugging

        response = requests.get(url, timeout=5)
        print(f"Status code: {response.status_code}")

        if response.status_code != 200:
            print(f"Failed with status {response.status_code}")
            if response.text:
                print("Response preview:", repr(response.text[:200]))
            return None

        print(f"Content-Type: {response.headers.get('Content-Type')}")

        config = parse_config(response.text)
        if config:
            print("Successfully parsed JSON config")
            _discovery_cache[cache_key] = (config, time.time())
            return config
        else:
            print("Parsed config is empty")
            return None

    except requests.RequestException as e:
        print(f"Request failed for {domain}/{server_type}: {e}")
        return None
    except Exception as e:
        print(f"Discovery error for {domain}/{server_type}: {e}")
        return None


def get_endpoint(domain: str, server_type: str, endpoint_name: str):
    """Get a specific endpoint URL from the discovered server config."""
    config = discover_server(domain, server_type)
    if not config:
        return None

    api = config.get("api")
    if not api:
        print("No 'api' section found in config")
        return None

    endpoints = api.get("endpoints")
    if not endpoints or endpoint_name not in endpoints:
        print(f"Endpoint '{endpoint_name}' not found")
        return None

    path = endpoints[endpoint_name]
    base = api.get("base", f"http://{domain}")

    # Clean URL joining
    if base.endswith("/") and path.startswith("/"):
        path = path[1:]
    elif not base.endswith("/") and not path.startswith("/"):
        base += "/"

    full_url = (base + path).rstrip("/")
    print(f"Resolved endpoint '{endpoint_name}' -> {full_url}")
    return full_url