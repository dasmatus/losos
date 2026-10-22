"""Draw a stand-in camera picture: a drawn person in a drawn room, 640x480."""
import subprocess
import sys

ROOMS = {
    "window": [
        "-fill", "#cfe3ee", "-draw", "rectangle 430,60 600,230",
        "-stroke", "#f4f1ea", "-strokewidth", "8", "-fill", "none",
        "-draw", "rectangle 430,60 600,230", "-draw", "line 515,60 515,230",
        "-draw", "line 430,145 600,145", "-stroke", "none",
    ],
    "shelf": [
        "-fill", "#6b4a2f", "-draw", "rectangle 40,150 220,162", "-draw", "rectangle 40,250 220,262",
        "-fill", "#c0392b", "-draw", "rectangle 55,100 75,150",
        "-fill", "#2e86de", "-draw", "rectangle 80,110 98,150",
        "-fill", "#f1c40f", "-draw", "rectangle 103,95 125,150",
        "-fill", "#27ae60", "-draw", "rectangle 60,205 90,250",
        "-fill", "#8e44ad", "-draw", "rectangle 95,200 115,250",
    ],
    "plant": [
        "-fill", "#8d5a3b", "-draw", "rectangle 500,330 580,420",
        "-fill", "#3c8d40", "-draw", "ellipse 540,290 60,70 0,360",
        "-fill", "#2f7a34", "-draw", "ellipse 510,270 30,50 0,360", "-draw", "ellipse 570,265 30,55 0,360",
        "-fill", "#f6e7b4", "-draw", "ellipse 120,110 40,40 0,360",
    ],
}


def hair(style, colour, skin):
    if style == "short":
        return [], ["-fill", colour, "-draw", "ellipse 320,150 92,62 180,360", "-draw", "ellipse 320,138 86,40 0,360"]
    if style == "long":
        return (["-fill", colour, "-draw", "roundrectangle 218,120 422,400 90,90"],
                ["-fill", colour, "-draw", "ellipse 320,148 95,66 180,360",
                 "-draw", "ellipse 290,140 70,38 0,360", "-draw", "ellipse 352,140 66,36 0,360"])
    # Short hair and a beard.
    return [], ["-fill", colour, "-draw", "ellipse 320,145 90,52 180,360", "-draw", "ellipse 320,135 82,34 0,360",
                "-draw", "ellipse 320,262 76,58 0,180", "-fill", skin, "-draw", "ellipse 320,252 40,18 0,360"]


def main():
    magick, out, wall, floor, skin, colour, shirt, style, room = sys.argv[1:]
    back, front = hair(style, colour, skin)
    args = [magick, "-size", "640x480", f"xc:{wall}",
            "-fill", floor, "-draw", "rectangle 0,420 640,480", *ROOMS[room], *back,
            "-fill", shirt, "-draw", "roundrectangle 150,345 490,560 110,110",
            "-fill", skin, "-draw", "rectangle 292,280 348,360", "-draw", "ellipse 320,350 34,16 0,180",
            "-draw", "ellipse 233,215 16,26 0,360", "-draw", "ellipse 407,215 16,26 0,360",
            "-draw", "ellipse 320,210 86,106 0,360", *front,
            "-fill", "#2b2b2b", "-draw", "ellipse 290,212 8,10 0,360", "-draw", "ellipse 350,212 8,10 0,360",
            "-fill", "white", "-draw", "ellipse 292,209 3,3 0,360", "-draw", "ellipse 352,209 3,3 0,360",
            "-stroke", "#3a2a20", "-strokewidth", "5", "-fill", "none",
            "-draw", "arc 270,180 310,200 200,340", "-draw", "arc 330,180 370,200 200,340",
            "-stroke", "#9a4a3a", "-draw", "arc 292,240 348,280 20,160",
            "-stroke", "none", "-fill", "#e9a38f",
            "-draw", "ellipse 268,248 14,8 0,360", "-draw", "ellipse 372,248 14,8 0,360",
            "-blur", "0x0.6", out]
    subprocess.run(args, check=True)


main()
