FROM python:3.12-slim

WORKDIR /app

COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

COPY . .

RUN mkdir -p outputs

# CMD can be overridden; default runs the analyzer
CMD ["python", "analyzer.py"]
