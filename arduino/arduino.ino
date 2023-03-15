// #define ENCODER_DO_NOT_USE_INTERRUPTS
#include <Arduino.h>
#include <FastLED.h>
#include <Encoder.h>


#define BAUD_RATE 115200

#define ENCODERCLK 0
#define ENCODERDT 1
#define ENCODERSW 2

#define LED_PIN 3
#define NUM_LEDS 300

#define BOTTOM_LEFT 0
#define TOP_LEFT 96
#define TOP_RIGHT 204
#define BOTTOM_RIGHT 299
#define TOP_MIDDLE (TOP_LEFT + (TOP_RIGHT - TOP_LEFT) / 2)

#define MAX_BRIGHTNESS 64
#define DEFAULT_BRIGHTNESS 32
#define DEFAULT_COLOR CHSV(HUE_ORANGE, 255, 255)

// Serial communication stuff
enum class Message : uint8_t {
  OsuModeChange,
  OsuKeyState,
  OsuHp,
  OsuHit,
  OsuEnd
};


// State stuff
struct NormalStateData {};
struct OsuGameplayStateData {
  bool k1;
  bool k2;
  byte hp;
};

struct State {
  State() = default;
  enum class Type : uint8_t {
    Normal,
    OsuGameplay
  };

  Type type;
  CHSV default_color;
  union {
    NormalStateData normal;
    OsuGameplayStateData osu_gameplay;
  };
};

State state;
CRGB leds[NUM_LEDS];
Encoder enc(ENCODERDT, ENCODERCLK);

void setup() {
  Serial.begin(BAUD_RATE);
  pinMode(ENCODERSW, INPUT_PULLUP);
  FastLED.addLeds<WS2812B, LED_PIN, GRB>(leds, NUM_LEDS);
  FastLED.setBrightness(DEFAULT_BRIGHTNESS);

  state.type = State::Type::Normal;
  state.default_color = DEFAULT_COLOR;
  fill_solid(leds, NUM_LEDS, state.default_color);
  FastLED.show();
}

void loop() {
  auto encoder_sw_state = digitalRead(ENCODERSW);
  auto delta = enc.readAndReset();
  bool led_refresh = false;

  // Update global brightness regardless of state
  if (encoder_sw_state == HIGH && delta) {
    long brightness = FastLED.getBrightness();
    brightness += delta;
    if (brightness > MAX_BRIGHTNESS)
      brightness = MAX_BRIGHTNESS;
    if (brightness < 0)
      brightness = 0;
    FastLED.setBrightness(brightness);
    FastLED.show();
  }

  // Handle serial events
  while (Serial.available() > 0) {
    Message msg = (Message)Serial.read();
    switch (msg) {
      case Message::OsuKeyState:
        state.osu_gameplay.k1 = (bool)Serial.read();
        state.osu_gameplay.k2 = (bool)Serial.read();
        fill_solid(leds, NUM_LEDS / 2, state.osu_gameplay.k1 ? CRGB::Yellow : CRGB::Black);
        fill_solid(leds + NUM_LEDS / 2, NUM_LEDS / 2, state.osu_gameplay.k2 ? CRGB::Yellow : CRGB::Black);
        FastLED.show();
        break;
      case Message::OsuEnd:
        state.type = State::Type::Normal;
      case Message::OsuModeChange:
        byte mode = Serial.read();
        if (mode == 2 && state.type != State::Type::OsuGameplay) {
          state.type = State::Type::OsuGameplay;
          state.osu_gameplay.hp = 0;
          state.osu_gameplay.k1 = 0;
          state.osu_gameplay.k2 = 0;
          while (leds[0].getAverageLight() > 0) {
            fadeToBlackBy(leds, NUM_LEDS, 2);
            FastLED.show();
            delay(1);
          }
        } else if (mode != 2) {
          state.type = State::Type::Normal;
          fill_solid(leds, NUM_LEDS, state.default_color);
          FastLED.show();
        }
        break;
    }
  }


  // Update current state
  switch (state.type) {
    case State::Type::Normal:
      {
        if (delta) {
          if (encoder_sw_state == LOW) {
            state.default_color.hue += delta;
            fill_solid(leds, NUM_LEDS, state.default_color);
          }
          FastLED.show();
        }
      }
      break;
  }

  if (led_refresh) FastLED.show();
}
