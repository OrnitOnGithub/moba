import io

tile_size = 10
map_path = "assets/maps/map_maker.map"
# W = wall
# U = unbreakable_wall
map_drawing = """
........WW.....W...
...............WW..
..W............W...
.............WWW...
...................
....UW....W........
....W....WW........
....W..............
....W..............
........W.....W....
W.......WWW...W....
"""

map_file = ""
for y, line in enumerate(io.StringIO(map_drawing)):
    for x, char in enumerate(repr(line)):
        position = f"{x*tile_size}.0 {y*tile_size}.0\n"
        if char == 'W':
            map_file += "wall " + position
        if char == 'U':
            map_file += "unbreakablewall " + position
with open(map_path, 'w') as file:
    file.write(map_file)