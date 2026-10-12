package kabam.rotmg.storage {
import com.company.assembleegameclient.game.GameSprite;
import com.company.assembleegameclient.objects.GameObject;
import com.company.assembleegameclient.objects.Player;
import com.company.assembleegameclient.screens.TitleMenuOption;
import com.company.assembleegameclient.ui.LineBreakDesign;
import com.company.rotmg.graphics.DeleteXGraphic;
import com.company.ui.SimpleText;
import com.company.util.GraphicsUtil;
import com.company.util.MoreObjectUtil;
import com.gskinner.motion.GTween;

import flash.display.CapsStyle;
import flash.display.Graphics;

import flash.display.GraphicsPath;

import flash.display.GraphicsSolidFill;
import flash.display.GraphicsStroke;
import flash.display.IGraphicsData;
import flash.display.JointStyle;
import flash.display.LineScaleMode;

import flash.display.Sprite;

import flash.events.Event;
import flash.events.MouseEvent;
import flash.events.TimerEvent;
import flash.utils.Timer;
import com.company.assembleegameclient.ui.TextButton;
import flash.filters.DropShadowFilter;
import flash.filters.GlowFilter;
import flash.text.TextFieldAutoSize;

import io.decagames.rotmg.ui.buttons.SliceScalingButton;
import io.decagames.rotmg.ui.popups.header.PopupHeader;
import io.decagames.rotmg.ui.popups.modal.ModalPopup;
import io.decagames.rotmg.ui.texture.TextureParser;
import io.decagames.rotmg.ui.defaults.DefaultLabelFormat;

import kabam.rotmg.account.core.Account;
import kabam.rotmg.appengine.api.AppEngineClient;

import kabam.rotmg.core.StaticInjectorContext;
import kabam.rotmg.game.model.GameModel;
import kabam.rotmg.game.signals.AddTextLineSignal;

import org.osflash.signals.Signal;

public class PotionsView extends Sprite {

    public var gs_:GameSprite;
    internal var player:Player;
    internal var text_:String;

    public var storageContainer:Sprite;
    public var potionContainers:Vector.<PotionsContainer>;
    private var titleText_:SimpleText;
    private var closeButton_:Sprite;
    private var capacityText_:SimpleText;
    private var upgradeButton_:TextButton;
    // how many of each potion the storage holds (the server sends it after the eight counts)
    public var capacity:int = 50;
    public static const UPGRADE_COST:int = 500;
    public static const UPGRADE_AMOUNT:int = 50;
    private var lineBreak_:LineBreakDesign;

    private var outlineFill_:GraphicsSolidFill = new GraphicsSolidFill(0x484848,1);
    private var lineStyle_:GraphicsStroke = new GraphicsStroke(3,false,LineScaleMode.NORMAL,CapsStyle.NONE,JointStyle.ROUND,3,outlineFill_);
    private var backgroundFill_:GraphicsSolidFill = new GraphicsSolidFill(0x323232,1);
    private var path_:GraphicsPath = new GraphicsPath(new Vector.<int>(),new Vector.<Number>());
    private var graphicsData_:Vector.<IGraphicsData> = new <IGraphicsData>[lineStyle_,backgroundFill_,path_,GraphicsUtil.END_FILL,GraphicsUtil.END_STROKE];
    private var listClient_:AppEngineClient;

    public function PotionsView(gs:GameSprite) {
        this.x = 55;
        this.y = 105;
        this.storageContainer = this.drawStorage();
        this.potionContainers = new Vector.<PotionsContainer>();

        this.titleText_ = new SimpleText(24, 0xFFFFFF, false, 800, 0);
        this.titleText_.setBold(true);
        this.titleText_.setText("Potion Storage");
        this.titleText_.autoSize = TextFieldAutoSize.LEFT;
        this.titleText_.x = 170;
        this.titleText_.y = 10;
        this.storageContainer.addChild(this.titleText_);

        // the X sits in a sprite with a solid (invisible) hit area, so a click anywhere on it counts
        var xIcon:DeleteXGraphic = new DeleteXGraphic();
        this.closeButton_ = new Sprite();
        this.closeButton_.graphics.beginFill(0, 0);
        this.closeButton_.graphics.drawRect(-4, -4, xIcon.width + 8, xIcon.height + 8);
        this.closeButton_.graphics.endFill();
        this.closeButton_.addChild(xIcon);
        this.closeButton_.buttonMode = true;
        this.closeButton_.x = 480 - xIcon.width;
        this.closeButton_.y = 10;
        this.closeButton_.addEventListener(MouseEvent.CLICK, onClose);
        this.storageContainer.addChild(this.closeButton_);

        this.capacityText_ = new SimpleText(16, 0xB3B3B3, false, 300, 0);
        this.capacityText_.x = 20;
        this.capacityText_.y = 378;
        this.storageContainer.addChild(this.capacityText_);

        this.upgradeButton_ = new TextButton(16, "Upgrade +" + UPGRADE_AMOUNT + " (" + UPGRADE_COST + " fame)", 210);
        this.upgradeButton_.x = 490 - 210 - 20;
        this.upgradeButton_.y = 372;
        this.upgradeButton_.addEventListener(MouseEvent.CLICK, onUpgrade);
        this.storageContainer.addChild(this.upgradeButton_);

        this.lineBreak_ = new LineBreakDesign(225, 0x242424);
        this.lineBreak_.scaleX = this.lineBreak_.scaleY = 2;
        this.lineBreak_.x = 20;
        this.lineBreak_.y = 48;
        this.storageContainer.addChild(this.lineBreak_);

        this.gs_ = gs;
        this.listClient_ = StaticInjectorContext.getInjector().getInstance(AppEngineClient);
        this.listClient_.setMaxRetries(2);
        this.requestPotions();
    }

    private function requestPotions():void
    {
        var account:Account = StaticInjectorContext.getInjector().getInstance(Account);
        var params:Object = {"num": this.gs_, "offset": 15};
        MoreObjectUtil.addToObject(params, account.getCredentials());
        this.listClient_.complete.addOnce(this.onComplete);
        this.listClient_.sendRequest("/account/grabPotions", params);
    }

    private function onComplete(_arg1:Boolean, _arg2:*):void
    {
        this.getAssets(_arg2);
    }

    private function getAssets(_arg1:String):void
    {
        this.setAssets(XML(_arg1));
    }

    private function setAssets(_arg1:XML): void
    {
        var potions:String = _arg1;
        var parsed:Array = potions.split(",");
        this.capacity = parsed.length > 8 ? int(parsed[8]) : 50;
        this.capacityText_.setText("Holds " + this.capacity + " of each potion");
        for each(var old:PotionsContainer in this.potionContainers)
        {
            if (old.parent != null)
                old.parent.removeChild(old);
        }
        this.potionContainers.length = 0;
        for(var i:int = 0; i < 8; i++)
        {
            var container:PotionsContainer = new PotionsContainer(this, this.gs_.map.player_, i, parsed[i]);
            container.x = 13 + (container.width * int(i % 4)) + (i < 4 ? 8 * i : 8 * (i - 4));
            container.y = i < 4 ? 65 : 215;
            this.storageContainer.addChild(container);
            this.potionContainers.push(container);
        }
    }

    public function drawStorage():Sprite
    {
        var b:Sprite = new Sprite;
        GraphicsUtil.clearPath(this.path_);
        GraphicsUtil.drawCutEdgeRect(0,0, 490, 420,8,[1,1,1,1],this.path_);
        b.graphics.drawGraphicsData(this.graphicsData_);
        addChild(b);
        return b;
    }

    public function draw():void {
        for (var i:int = 0; i < 8; i++)
            this.potionContainers[i].draw();
    }

    public function useStorage(type:int, action:int):void
    {
        this.gs_.gsc_.PotionInteraction(type, action);
    }

    // the server takes the fame and raises the capacity; then the counts and capacity are read again
    private function onUpgrade(e:Event):void {
        this.useStorage(0, 4);
        var t:Timer = new Timer(800, 1);
        t.addEventListener(TimerEvent.TIMER_COMPLETE, function(ev:TimerEvent):void {
            if (parent != null)
                requestPotions();
        });
        t.start();
    }

    public function onClose(arg1:Event):void {
        this.closeButton_.removeEventListener(MouseEvent.CLICK, onClose);
        this.upgradeButton_.removeEventListener(MouseEvent.CLICK, onUpgrade);
        if (parent != null)
            parent.removeChild(this);
    }
}
}
