import subprocess
import time
import sys
import os

VENV_PYTHON = os.path.abspath("venv/bin/python3")
SCRIPT_NAME = "fetch_bowie_data.py"

def main():
    if not os.path.exists(VENV_PYTHON):
        print(f"Error: {VENV_PYTHON} not found.")
        sys.exit(1)

    print(f"Starting exhaustive supervisor for {SCRIPT_NAME}...")
    
    while True:
        print(f"\n--- Starting {SCRIPT_NAME} at {time.ctime()} ---")
        process = subprocess.Popen(
            [VENV_PYTHON, "-u", SCRIPT_NAME],
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True
        )
        
        is_complete = False
        try:
            while True:
                line = process.stdout.readline()
                if not line:
                    break
                sys.stdout.write(line)
                sys.stdout.flush()
                if "Exhaustive fetch complete" in line:
                    is_complete = True
        except KeyboardInterrupt:
            process.terminate()
            sys.exit(0)
        
        return_code = process.wait()
        if is_complete:
            print("\nSupervisor: Exhaustive fetch finished successfully.")
            break
        
        print(f"\nSupervisor: Script exited with code {return_code}. Restarting in 30s...")
        time.sleep(30)

if __name__ == "__main__":
    main()
