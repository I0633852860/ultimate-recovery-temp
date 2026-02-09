import subprocess
import os

links = [
    "https://youtube.com/watch?v=--jewyPuxUI",
    "https://youtube.com/watch?v=-ofS1sLXZjM", 
    "https://youtube.com/watch?v=0VyuxjgMfGE",
    "https://youtube.com/watch?v=18rOK1sCzWI"
]

output_dir = "/home/yevgen/ultimate_recovery/semantic_training"

def download_samples():
    if not os.path.exists(output_dir):
        os.makedirs(output_dir)
        
    for i, link in enumerate(links):
        print(f"Processing {i+1}/{len(links)}: {link}")
        try:
            subprocess.run([
                "yt-dlp", 
                "--write-info-json",
                "--write-subs", 
                "--write-auto-sub",
                "--sub-lang", "en,ru", 
                "--skip-download",
                "-o", f"{output_dir}/%(title)s [%(id)s].%(ext)s",
                link
            ], check=True)
        except subprocess.CalledProcessError as e:
            print(f"Failed to download {link}: {e}")

if __name__ == "__main__":
    download_samples()
