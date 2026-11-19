"""Rebuild the SPEC-004 provisional sprite sheets and chime from local sources."""
from math import cos, sin, pi, radians, exp
from pathlib import Path
from struct import pack
import wave
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
ART = ROOT / "src/assets"
FRAME = 180
COUNT = 32


def character_parts():
    # Keep the chair, seated character and boots from the accepted placeholder.
    svg = ET.parse(ART / "placeholder.svg").getroot()
    layers = []
    for elem in svg:
        elem.tag = elem.tag.split("}")[-1]
        if elem.tag != "title":
            layers.append(ET.tostring(elem, encoding="unicode"))
    chair, legs, arms, shirt, *head_and_boots = layers
    assert "#d59273" in arms
    return chair + legs, shirt + "".join(head_and_boots)


def limb(shoulder, elbow, hand):
    coords = " ".join(f"{x:.2f},{y:.2f}" for x, y in (shoulder, elbow, hand))
    return f'<polyline points="{coords}" fill="none" stroke="#d59273" stroke-width="10" stroke-linecap="round" stroke-linejoin="round"/>'


def segment(start, length, angle):
    angle = radians(angle)
    return start[0] + length*cos(angle), start[1] + length*sin(angle)


def arms_up(progress):
    shoulder = (70, 69)
    elbow = segment(shoulder, 26, 111 + 139*progress)
    hand = segment(elbow, 27, 56 + 202*progress)
    left = (shoulder, elbow, hand)
    return limb(*left) + limb(*[(180-x, y) for x, y in left])


def drink(progress):
    elbow = (123+10*progress, 91-20*progress)
    hand = (119-10*progress, 104-53*progress)
    left = limb((70, 69), (62, 93), (84, 108))
    bottle = f'<use href="#bottle" transform="translate({hand[0]:.2f} {hand[1]:.2f}) rotate({-67*progress:.2f})"/>'
    return left + limb((109, 69), elbow, hand) + bottle


def sheet(name, pose):
    back, front = character_parts()
    bottle = """<g id="bottle">
      <path d="M-4-16h8v5l3 4v17q0 3-3 3h-8q-3 0-3-3V-7l3-4z" fill="#4586ce" stroke="#284a76" stroke-width="1.5"/>
      <path d="M-4-17h8v3h-8z" fill="#244a70"/><path d="M-4-3h8v5h-8z" fill="#a8dce9"/>
    </g>"""
    frames = []
    for i in range(COUNT):
        # Ease to the pose, hold it briefly, and return gently to rest.
        level = min(1, max(0, (.5 - .5*cos(2*pi*i/COUNT))*1.22-.10))
        frames.append(f'<g transform="translate({i*FRAME} 0)"><use href="#back"/><use href="#front"/>{pose(level)}</g>')
    (ART / f"{name}.svg").write_text(
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{FRAME*COUNT}" height="{FRAME}" viewBox="0 0 {FRAME*COUNT} {FRAME}" fill="none">\n'
        f'<title>{name}</title><defs><g id="back">{back}</g><g id="front">{front}</g>{bottle}</defs>\n'
        + "\n".join(frames) + "\n</svg>\n"
    )


def chime():
    rate, duration = 32000, .68
    folder = ROOT / "src-tauri/assets"
    folder.mkdir(exist_ok=True)
    samples = bytearray()
    for i in range(round(rate*duration)):
        t = i / rate
        attack = 1-exp(-t/.008)
        tail = min(1, (duration-t)/.055)
        value = (.11*exp(-t/.17)*sin(2*pi*860*t)
                 + .045*exp(-t/.115)*sin(2*pi*1290*t)
                 + .022*exp(-t/.075)*sin(2*pi*2236*t))
        samples += pack("<h", round(32767*attack*tail*value))
    with wave.open(str(folder / "cling.wav"), "wb") as out:
        out.setparams((1, 2, rate, 0, "NONE", "not compressed"))
        out.writeframes(samples)


if __name__ == "__main__":
    sheet("stretch-poses", arms_up)
    sheet("water-poses", drink)
    chime()
