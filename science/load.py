import threading
import requests


def send_request():
    response = requests.get("http://localhost:3055/v1/trips")
    response2 = requests.get("http://localhost:3055/v1/stop_times?bus_route_ids=B44")
    print(response.status_code, response2.status_code)


threads = []
for _ in range(1000):
    thread = threading.Thread(target=send_request)
    thread.start()
    threads.append(thread)

for thread in threads:
    thread.join()
