import cv2 as cv
import imutils
import time
import math
import redis
import numpy as np


def drawDetection(int, x, cv, frame, y, radius):
    cv.circle(frame, (int(x), int(y)), int(radius), (255, 255, 255), 2)
    cv.circle(frame, (int(x), int(y)), 5, (0, 0, 255), -1)


def prepareFrame(colorLower, colorUpper, frameSize, cv):
    hsv = cv.cvtColor(frame, cv.COLOR_BGR2HSV)
    mask = cv.inRange(hsv, colorLower, colorUpper)
    mask = cv.erode(mask, None, iterations=2)
    mask = cv.dilate(mask, None, iterations=2)
    return mask


def make_relative(x, y):
    table_left = 40
    table_right = 1170
    table_width = table_right - table_left
    table_top = 50
    table_bottom = 710
    table_height = table_bottom - table_top

    x = (x - table_left) / table_width
    y = (y - table_top) / table_height

    if x > 1.0:
        x = 1.0
    elif x < 0.0:
        x = 0.0

    if y > 1.0:
        y = 1.0
    elif y < 0.0:
        y = 0.0

    return (x, 1.0 - y)


def make_player_relative(kind: str, position: float):
    min_max_map = {
        "gk": (0.335, 0.686),
        "defence": (0.331, 0.688),
        "midfield": (0.219, 0.556),
        "striker": (0.331, 0.688),
    }

    kind = kind.removeprefix("opp_")

    min, max = min_max_map[kind]
    position = (position - min) / (max - min)

    if position > 1.0:
        position = 1.0
    elif position < 0.0:
        position = 0.0

    return position


def calculate_angle(kind: str, radius: float, x: float):
    min_max_radius_map = {
        "midfield": (17.9, 54.2),
        "striker": (17.9, 55.9),
        "defence": (18.6, 57.4),
        "gk": (19.0, 60.7)
    }

    clipped_kind = kind.removeprefix("opp_")
    min, max = min_max_radius_map[clipped_kind]

    angle = math.degrees(math.atan((radius - max) / (radius - min))) + 90.0

    center_map = {
        "gk": 0.062,
        "defence": 0.187,
        "opp_striker": 0.310,
        "midfield": 0.436,
        "opp_midfield": 0.560,
        "striker": 0.684,
        "opp_defence": 0.807,
        "opp_gk": 0.935,
    }

    sign = 0
    center = center_map[kind]
    if kind.startswith("opp_"):
        sign = 1 if x < center else -1
    else:
        sign = 1 if x > center else -1
    signed_angle = angle * sign

    offset_map = {
        "midfield": math.atan(4.75 / 95),
        "striker": -math.atan((4.75 + 9.5) / 95),
        "defence": -math.atan((4.75 + 9.5 * 2) / 95),
        "gk": -math.atan((4.75 + 9.5 * 3) / 95),
    }

    return signed_angle + math.degrees(offset_map[clipped_kind])


# redis = redis.Redis(host='localhost', port=6379, db=0)


# define framevars
frameSize = 800

cap = cv.VideoCapture(2, cv.CAP_V4L2)
cap.set(cv.CAP_PROP_FRAME_WIDTH, 1280)
cap.set(cv.CAP_PROP_FRAME_HEIGHT, 960)
cap.set(cv.CAP_PROP_FOURCC, cv.VideoWriter_fourcc('M', 'J', 'P', 'G'))
cap.set(cv.CAP_PROP_FPS, 30)

frame_count = 0
starttime = time.time()
while True:
    # Capture frame-by-frame
    ret, frame = cap.read()
    if not ret:
        print("failed to capture")
        continue
    frame_count += 1
    if (time.time() - starttime) > 1.0:
        print(frame_count / (time.time() - starttime))
        print(frame_count)
        frame_count = 0
        starttime = time.time()

    # frame = imutils.resize(frame, width=frameSize)
    frame = imutils.rotate(frame, 1)
    frame = frame[100:-100, 90:]
    ball_mask = prepareFrame((160, 100, 100), (180, 255, 255), frameSize, cv)
    player_mask = prepareFrame((95, 100, 100), (105, 255, 255), frameSize, cv)
    blue_player_mask = prepareFrame((90, 100, 100), (110, 255, 255), frameSize, cv)
    yellow_player_mask = prepareFrame((10, 100, 100), (40, 255, 255), frameSize, cv)

    position = (-1, -1)

    cnts = cv.findContours(ball_mask.copy(), cv.RETR_EXTERNAL, cv.CHAIN_APPROX_SIMPLE)[-2]
    blue_players = cv.findContours(blue_player_mask.copy(), cv.RETR_EXTERNAL, cv.CHAIN_APPROX_SIMPLE)[-2]
    yellow_players = cv.findContours(yellow_player_mask.copy(), cv.RETR_EXTERNAL, cv.CHAIN_APPROX_SIMPLE)[-2]

    if len(cnts) > 0:
        cs = sorted(cnts, key=cv.contourArea)
        ((x, y), radius) = cv.minEnclosingCircle(cs[-1])
        ((other_x, other_y), other_radius) = \
            cv.minEnclosingCircle(cs[-2]) if len(cs) > 1 else ((None, None), None)

        if other_radius is not None and radius / other_radius < 1.5:
            x, y = ((x + other_x) / 2, (y + other_y) / 2)

        if radius > 1:
            drawDetection(int, x, cv, frame, y, radius)
            x, y = make_relative(x, y)
            # redis.set("ball_x", x)
            # redis.set("ball_y", y)

    points_dict = {}

    for player in yellow_players:
        ((x, y), radius) = cv.minEnclosingCircle(player)

        if radius > 5:
            drawDetection(int, x, cv, frame, y, radius)

            x, y = make_relative(x, y)
            if x < 0.14:
                points_dict["gk"] = [(x, y, radius)]
            elif x < 0.26:
                if "defence" not in points_dict:
                    points_dict["defence"] = [(x, y, radius)]
                else:
                    points_dict["defence"].append((x, y, radius))
            elif x < 0.50:
                if "midfield" not in points_dict:
                    points_dict["midfield"] = [(x, y, radius)]
                else:
                    points_dict["midfield"].append((x, y, radius))
            else:
                if "striker" not in points_dict:
                    points_dict["striker"] = [(x, y, radius)]
                else:
                    points_dict["striker"].append((x, y, radius))

    for player in blue_players:
        ((x, y), radius) = cv.minEnclosingCircle(player)

        if radius > 5:
            drawDetection(int, x, cv, frame, y, radius)

            x, y = make_relative(x, y)
            if x < 0.38:
                if "opp_striker" not in points_dict:
                    points_dict["opp_striker"] = [(x, y, radius)]
                else:
                    points_dict["opp_striker"].append((x, y, radius))
            elif x < 0.62:
                if "opp_midfield" not in points_dict:
                    points_dict["opp_midfield"] = [(x, y, radius)]
                else:
                    points_dict["opp_midfield"].append((x, y, radius))
            elif x < 0.86:
                if "opp_defence" not in points_dict:
                    points_dict["opp_defence"] = [(x, y, radius)]
                else:
                    points_dict["opp_defence"].append((x, y, radius))
            else:
                points_dict["opp_gk"] = [(x, y, radius)]

    for kind, points in points_dict.items():
        x, y, radius = sorted(points, key=lambda point: point[1])[1] if len(points) > 1 else points[0]
        position = make_player_relative(kind, y)
        angle = calculate_angle(kind, radius, x)
        # redis.set(kind + "_position", position)
        # redis.set(kind + "_angle", angle)

    # redis.set("last_update", time.time_ns())

    cv.imshow('frame', frame)

    if cv.waitKey(1) & 0xFF == ord('q'):
        break

# When everything done, release the capture
cap.release()
cv.destroyAllWindows()
