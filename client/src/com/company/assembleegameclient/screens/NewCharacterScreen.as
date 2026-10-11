package com.company.assembleegameclient.screens
{
   import com.company.assembleegameclient.appengine.SavedCharactersList;
   import com.company.assembleegameclient.objects.ObjectLibrary;
   import com.company.rotmg.graphics.ScreenGraphic;
import com.company.ui.SimpleText;

import flash.display.Graphics;
import flash.display.Shape;
import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.MouseEvent;
import flash.filters.DropShadowFilter;

import io.decagames.rotmg.ui.buttons.SliceScalingButton;
import io.decagames.rotmg.ui.defaults.DefaultLabelFormat;
import io.decagames.rotmg.ui.sliceScaling.SliceScalingBitmap;
import io.decagames.rotmg.ui.texture.TextureParser;

import kabam.rotmg.core.model.PlayerModel;
   import kabam.rotmg.game.view.CreditDisplay;
   import kabam.rotmg.ui.view.components.ScreenBase;
   import org.osflash.signals.Signal;

   public class NewCharacterScreen extends Sprite
   {
      private var backButton_:TitleMenuOption;
      private var creditDisplay_:CreditDisplay;
      private var boxes_:Object;
      public var tooltip:Signal;
      public var close:Signal;
      public var selected:Signal;
      private var title:SimpleText;
      private var graphic:Sprite;
      private var lines:Sprite;
      private var container:Sprite;

      private var isInitialized:Boolean = false;

      private var gridWidth:Number = 705;
      private var gridHeight:Number = 105;

      public function NewCharacterScreen()
      {
         this.boxes_ = {};
         super();
         this.tooltip = new Signal(Sprite);
         this.selected = new Signal(int);
         this.close = new Signal();
         addChild(new ScreenBase());
         addChild(new AccountScreen());
      }

      private function makeBar():Sprite
      {
         var box:Sprite = new Sprite();
         var b:Graphics = box.graphics;
         b.clear();
         b.beginFill(0, 0.5);
         b.drawRect(0, 0, 800, 75);
         b.endFill();
         addChild(box);
         return box;
      }

      private function makeTitleText() : void
      {
         this.title = new SimpleText(32,11776947,false,0,0);
         this.title.setBold(true);
         this.title.text = "Classes";
         this.title.updateMetrics();
         this.title.filters = [new DropShadowFilter(0,0,0,1,8,8)];
         this.title.x = 400 - this.title.width / 2;
         this.title.y = 24;
         addChild(this.title);
      }

      private function positionButtons(e:Event = null) : void
      {
         if (e != null)
         {
            ScreenBase.reSize(e);
            AccountScreen.reSize(e);
         }

         var width:int = WebMain.STAGE.stageWidth;
         var height:int = WebMain.STAGE.stageHeight;
         this.lines.width = width;
         this.graphic.width = width;
         this.graphic.y = height - 75;
         // scale the class grid down when it doesn't fit between the title line and the bottom bar
         var scale:Number = Math.min(1, (width - 20) / this.gridWidth, (height - 110 - 85) / this.gridHeight);
         this.container.scaleX = this.container.scaleY = scale;
         this.container.x = (width - this.gridWidth * scale) / 2;
         this.container.y = 110;
         this.creditDisplay_.x = width;
         this.title.x = (width / 2) - (this.title.width / 2);
         this.backButton_.x = (width / 2) - (this.backButton_.width / 2);
         this.backButton_.y = height - 70;
      }

      private function drawLines():Sprite
      {
         var box:Sprite = new Sprite();
         var b:Graphics = box.graphics;
         b.clear();
         b.lineStyle(2,6184542);
         b.moveTo(0,100);
         b.lineTo(800,100);
         b.lineStyle();
         addChild(box);
         return box;
      }

      public function initialize(model:PlayerModel) : void
      {
         var playerXML:XML = null;
         var objectType:int = 0;
         var characterType:String = null;
         var overrideIsAvailable:Boolean = false;
         var charBox:CharacterBox = null;
         if(this.isInitialized)
         {
            return;
         }
         this.isInitialized = true;
         this.graphic = this.makeBar();
         addChild(this.graphic);
         this.makeTitleText();
         this.backButton_ = new TitleMenuOption("back",36,false);
         this.backButton_.addEventListener(MouseEvent.CLICK,this.onBackClick);
         addChild(this.backButton_);
         this.creditDisplay_ = new CreditDisplay();
         this.creditDisplay_.draw(model.getCredits(),model.getFame());
         addChild(this.creditDisplay_);
         this.creditDisplay_.y = 32;
         this.container = new Sprite();
         addChild(this.container);
         var shown:Array = [];
         for(var i:int = 0; i < ObjectLibrary.playerChars_.length; i++)
         {
            if(!model.isClassAvailability(String(ObjectLibrary.playerChars_[i].@id),SavedCharactersList.UNAVAILABLE))
               shown.push(ObjectLibrary.playerChars_[i]);
         }
         // at least 6 columns, more when needed to keep it to 4 rows; the last row is centred
         var cols:int = Math.max(6, Math.ceil(shown.length / 4));
         var rows:int = Math.max(1, Math.ceil(shown.length / cols));
         this.gridWidth = cols * 120 - 15;
         this.gridHeight = rows * 120 - 15;
         for(i = 0; i < shown.length; i++)
         {
            playerXML = shown[i];
            objectType = int(playerXML.@type);
            characterType = playerXML.@id;
            {
               overrideIsAvailable = model.isClassAvailability(characterType,SavedCharactersList.UNRESTRICTED);
               charBox = new CharacterBox(playerXML,model.getCharStats()[objectType],model,overrideIsAvailable);
               var row:int = int(i / cols);
               var inRow:int = row == rows - 1 ? shown.length - row * cols : cols;
               charBox.x = 120 * (i % cols) + (cols - inRow) * 60;
               charBox.y = 120 * row;
               this.boxes_[objectType] = charBox;
               charBox.addEventListener(MouseEvent.ROLL_OVER,this.onCharBoxOver);
               charBox.addEventListener(MouseEvent.ROLL_OUT,this.onCharBoxOut);
               charBox.characterSelectClicked_.add(this.onCharBoxClick);
               this.container.addChild(charBox);
            }
         }

         this.lines = drawLines();
         addChild(this.lines);

         this.positionButtons();
         if (WebMain.STAGE)
             WebMain.STAGE.addEventListener(Event.RESIZE, positionButtons);
      }

      private function onBackClick(event:Event) : void
      {
         this.close.dispatch();
      }

      private function onCharBoxOver(event:MouseEvent) : void
      {
         var charBox:CharacterBox = event.currentTarget as CharacterBox;
         charBox.setOver(true);
         this.tooltip.dispatch(charBox.getTooltip());
      }

      private function onCharBoxOut(event:MouseEvent) : void
      {
         var charBox:CharacterBox = event.currentTarget as CharacterBox;
         charBox.setOver(false);
         this.tooltip.dispatch(null);
      }

      private function onCharBoxClick(event:MouseEvent) : void
      {
         this.tooltip.dispatch(null);
         var charBox:CharacterBox = event.currentTarget.parent as CharacterBox;
         if(!charBox.available_)
         {
            return;
         }
         var objectType:int = charBox.objectType();
         var displayId:String = ObjectLibrary.typeToDisplayId_[objectType];
         this.selected.dispatch(objectType);
      }

      public function updateCreditsAndFame(credits:int, fame:int) : void
      {
         this.creditDisplay_.draw(credits,fame);
      }

      public function update(model:PlayerModel) : void
      {
         var playerXML:XML = null;
         var objectType:int = 0;
         var characterType:String = null;
         var overrideIsAvailable:Boolean = false;
         var charBox:CharacterBox = null;
         for(var i:int = 0; i < ObjectLibrary.playerChars_.length; i++)
         {
            playerXML = ObjectLibrary.playerChars_[i];
            objectType = int(playerXML.@type);
            characterType = String(playerXML.@id);
            if(!model.isClassAvailability(characterType,SavedCharactersList.UNAVAILABLE))
            {
               overrideIsAvailable = model.isClassAvailability(characterType,SavedCharactersList.UNRESTRICTED);
               charBox = this.boxes_[objectType];
               if(charBox)
               {
                  if(overrideIsAvailable || model.isLevelRequirementsMet(objectType))
                  {
                     charBox.unlock();
                  }
               }
            }
         }
      }
   }
}
