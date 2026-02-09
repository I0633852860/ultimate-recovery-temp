
import json
import os
import subprocess

def download_subs(link_file, output_dir, limit=20):
    with open(link_file, 'r') as f:
        data = json.load(f)
    
    links = data.get('links', [])[:limit]
    
    for i, link in enumerate(links):
        print(f"Processing {i+1}/{limit}: {link}")
        # Download metadata and subtitles (auto-generated or manual)
        # --write-auto-sub: write automatically generated subtitles
        # --sub-lang en,ru: preferred languages
        # --skip-download: distinct from video
        # --write-info-json: metadata
        try:
            subprocess.run([
                "yt-dlp", 
                "--write-info-json",
                "--write-subs", 
                "--write-auto-sub",
                "--sub-lang", "en,ru", 
                "--skip-download",
                "--ignore-errors",  # Continue even if download errors occur
                "--no-check-certificate",
                "--force-ipv4",     # Often fixes connection issues
                "--user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36",
                "--sleep-interval", "3", # Avoid rate limiting
                "-o", f"{output_dir}/%(title)s [%(id)s].%(ext)s",
                link
            ], check=True)
        except subprocess.CalledProcessError as e:
            print(f"Failed to download {link}: {e}")

if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument("--limit", type=int, default=20, help="Limit number of downloads")
    parser.add_argument("--all", action="store_true", help="Download all links")
    parser.add_argument("--output", default="semantic_training", help="Output directory")
    args = parser.parse_args()

    limit = 999999 if args.all else args.limit
    
    # Create dir if not exists
    os.makedirs(args.output, exist_ok=True)
    
    link_file = "all_links.json"
    if not os.path.exists(link_file) and os.path.exists("../all_links.json"):
        link_file = "../all_links.json"
    
    download_subs(link_file, args.output, limit=limit)
