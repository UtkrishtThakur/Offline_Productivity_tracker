class SessionMemory:
    def __init__(self):
        self.projects = {}
        self.total_time = 0

    def update(self, formatted_data):
        self.projects = formatted_data

        for project in formatted_data.values():
            self.total_time += project.get("time_min", 0)