"""KDL parser and server discovery for BSCP"""
import requests


def parse_kdl(kdl_text):
    """Simple KDL parser for BSCP config"""
    config = {}
    lines = kdl_text.strip().split('\n')
    current_section = None
    current_subsection = None

    for line in lines:
        line = line.strip()
        if not line or line.startswith('//'):
            continue

        # Handle section start: key {
        if '{' in line and not '}' in line:
            section_name = line.split('{')[0].strip()
            current_section = section_name
            config[section_name] = {}
            continue

        # Handle end of section
        if '}' in line:
            current_section = None
            current_subsection = None
            continue

        # Handle subsections: key {
        if '{' in line and current_section:
            subsection_name = line.split('{')[0].strip()
            current_subsection = subsection_name
            config[current_section][subsection_name] = {}
            continue

        # Handle key-value pairs
        if current_subsection:
            if ' ' in line:
                key, value = line.split(None, 1)
                value = value.strip('"')
                if current_subsection not in config[current_section]:
                    config[current_section][current_subsection] = {}
                config[current_section][current_subsection][key] = value
        elif current_section:
            if ' ' in line:
                key, value = line.split(None, 1)
                value = value.strip('"')
                config[current_section][key] = value

    return config


def discover_server(domain, server_type="userserver"):
    """Fetch and parse .well-known config from a domain"""
    try:
        url = f"http://{domain}/.well-known/BSCP/{server_type}.kdl"
        response = requests.get(url, timeout=3)
        if response.status_code == 200:
            return parse_kdl(response.text)
    except Exception as e:
        print(f"Discovery failed for {domain}/{server_type}: {e}")
    return None


def get_endpoint(domain, server_type, endpoint_name):
    """Get a specific endpoint URL from a discovered server"""
    config = discover_server(domain, server_type)
    if config and 'api' in config and 'endpoints' in config['api']:
        endpoint_path = config['api']['endpoints'].get(endpoint_name)
        if endpoint_path:
            base = config['api'].get('base', f"http://{domain}")
            return f"{base}{endpoint_path}"
    return None
