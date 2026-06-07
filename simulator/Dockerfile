FROM python:3.11-slim

WORKDIR /app

COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

COPY ethercat_simulator.py .

CMD ["python", "ethercat_simulator.py"]
